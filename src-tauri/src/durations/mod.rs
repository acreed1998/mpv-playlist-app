pub mod cache;
pub mod probe;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;

pub use cache::{Cache, Fingerprint};

// How many ffprobe processes run at once. Each is mostly waiting on disk,
// so a few in parallel is a good speedup without hammering a spinning drive.
const WORKERS: usize = 4;

pub struct Outcome {
    pub durations: HashMap<String, f64>, // only files whose length is known
    pub cancelled: bool,
}

type ProbeResult = Result<Option<f64>, String>;

// Find the length of every file, using the cache where it's still valid and
// probing the rest in parallel. `on_progress(done, total)` counts only files
// that needed probing.
pub fn ensure_durations<P>(
    paths: &[String],
    cache: &mut Cache,
    cancel: &AtomicBool,
    probe: P,
    mut on_progress: impl FnMut(usize, usize),
) -> Result<Outcome, String>
where
    P: Fn(&str) -> ProbeResult + Sync,
{
    let mut durations = HashMap::new();
    let mut todo: Vec<(String, Fingerprint)> = Vec::new();
    let mut seen = HashSet::new();

    // Phase 1 (one thread): answer from the cache wherever possible.
    for path in paths {
        if !seen.insert(path.as_str()) {
            continue; // the same file listed twice is probed once
        }
        // No fingerprint means we can't stat it (missing, not a file): unknown.
        let Some(fingerprint) = Fingerprint::of(path) else { continue };

        match cache.lookup(path, fingerprint) {
            Some(Some(secs)) => {
                durations.insert(path.clone(), secs);
            }
            Some(None) => {} // cached as unreadable: leave it unknown
            None => todo.push((path.clone(), fingerprint)),
        }
    }

    let total = todo.len();
    on_progress(0, total);
    if total == 0 {
        return Ok(Outcome { durations, cancelled: false });
    }

    // Phase 2: probe the rest on a few threads.
    let next = AtomicUsize::new(0); // the next unclaimed index in `todo`
    let abort = AtomicBool::new(false); // set when ffprobe can't run at all
    let mut failure: Option<String> = None;
    let mut done = 0;
    let (tx, rx) = mpsc::channel::<(usize, ProbeResult)>();

    thread::scope(|scope| {
        for _ in 0..WORKERS.min(total) {
            let tx = tx.clone();
            let (todo, next, abort, probe) = (&todo, &next, &abort, &probe);
            scope.spawn(move || loop {
                if cancel.load(Ordering::SeqCst) || abort.load(Ordering::SeqCst) {
                    break;
                }
                // fetch_add hands each worker a different index.
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some((path, _)) = todo.get(i) else { break };
                if tx.send((i, probe(path.as_str()))).is_err() {
                    break;
                }
            });
        }
        drop(tx); // keep only the workers' copies, so the loop below can end

        for (i, result) in rx {
            match result {
                Ok(duration) => {
                    let (path, fingerprint) = &todo[i];
                    cache.insert(path.clone(), *fingerprint, duration);
                    if let Some(secs) = duration {
                        durations.insert(path.clone(), secs);
                    }
                    done += 1;
                    on_progress(done, total);
                }
                Err(message) => {
                    abort.store(true, Ordering::SeqCst);
                    if failure.is_none() {
                        failure = Some(message);
                    }
                }
            }
        }
    });

    if let Some(message) = failure {
        return Err(message);
    }
    // The only way to stop early without a failure is the cancel flag.
    Ok(Outcome { durations, cancelled: done < total })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp_files(names: &[&str]) -> (PathBuf, Vec<String>) {
        let root = std::env::temp_dir()
            .join(format!("mpv-dur-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let paths = names
            .iter()
            .map(|name| {
                let path = root.join(name);
                fs::write(&path, name.as_bytes()).unwrap();
                path.to_str().unwrap().to_string()
            })
            .collect();
        (root, paths)
    }

    #[test]
    fn probes_once_then_serves_from_the_cache_and_reports_progress() {
        let (root, paths) = temp_files(&["a.mp4", "b.mp4", "c.mp4"]);
        let mut cache = Cache::default();
        let cancel = AtomicBool::new(false);
        let calls = AtomicUsize::new(0);
        let probe = |path: &str| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(Some(path.len() as f64))
        };

        let mut seen = Vec::new();
        let first =
            ensure_durations(&paths, &mut cache, &cancel, &probe, |d, t| seen.push((d, t))).unwrap();
        assert_eq!(first.durations.len(), 3);
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        assert_eq!(seen.first(), Some(&(0, 3)));
        assert_eq!(seen.last(), Some(&(3, 3)));

        let second = ensure_durations(&paths, &mut cache, &cancel, &probe, |_, _| {}).unwrap();
        assert_eq!(second.durations.len(), 3);
        assert_eq!(calls.load(Ordering::SeqCst), 3); // nothing probed again

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_changed_file_is_probed_again() {
        let (root, paths) = temp_files(&["a.mp4", "b.mp4"]);
        let mut cache = Cache::default();
        let cancel = AtomicBool::new(false);
        let calls = AtomicUsize::new(0);
        let probe = |_: &str| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(Some(1.0))
        };

        ensure_durations(&paths, &mut cache, &cancel, &probe, |_, _| {}).unwrap();
        fs::write(&paths[0], b"a much longer file than before").unwrap(); // size changes

        ensure_durations(&paths, &mut cache, &cancel, &probe, |_, _| {}).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 3); // 2 first time, +1 for the changed file

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn unreadable_files_stay_unknown_and_are_not_retried() {
        let (root, paths) = temp_files(&["good.mp4", "bad.mp4"]);
        let mut cache = Cache::default();
        let cancel = AtomicBool::new(false);
        let calls = AtomicUsize::new(0);
        let probe = |path: &str| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(if path.contains("bad") { None } else { Some(5.0) })
        };

        let outcome = ensure_durations(&paths, &mut cache, &cancel, &probe, |_, _| {}).unwrap();
        assert_eq!(outcome.durations.len(), 1);
        assert!(outcome.durations.contains_key(&paths[0]));

        ensure_durations(&paths, &mut cache, &cancel, &probe, |_, _| {}).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2); // bad.mp4 was not retried

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn duplicates_are_probed_once_and_missing_files_are_skipped() {
        let (root, paths) = temp_files(&["a.mp4"]);
        let listed = vec![paths[0].clone(), paths[0].clone(), "/no/such/file.mp4".to_string()];
        let mut cache = Cache::default();
        let cancel = AtomicBool::new(false);
        let calls = AtomicUsize::new(0);
        let probe = |_: &str| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(Some(9.0))
        };

        let outcome = ensure_durations(&listed, &mut cache, &cancel, &probe, |_, _| {}).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(outcome.durations.len(), 1);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_missing_tool_fails_the_whole_job() {
        let (root, paths) = temp_files(&["a.mp4", "b.mp4", "c.mp4"]);
        let mut cache = Cache::default();
        let cancel = AtomicBool::new(false);
        let probe = |_: &str| Err("ffprobe was not found on your PATH".to_string());

        let result = ensure_durations(&paths, &mut cache, &cancel, &probe, |_, _| {});
        assert!(result.err().unwrap().contains("not found"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn cancelling_stops_before_probing() {
        let (root, paths) = temp_files(&["a.mp4", "b.mp4"]);
        let mut cache = Cache::default();
        let cancel = AtomicBool::new(true); // already cancelled
        let calls = AtomicUsize::new(0);
        let probe = |_: &str| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(Some(1.0))
        };

        let outcome = ensure_durations(&paths, &mut cache, &cancel, &probe, |_, _| {}).unwrap();
        assert!(outcome.cancelled);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(outcome.durations.is_empty());

        let _ = fs::remove_dir_all(&root);
    }
}
