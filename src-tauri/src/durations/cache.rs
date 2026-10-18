use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

// "Is this still the same file we measured?"
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Fingerprint {
    pub size: u64,
    pub modified_ms: u64,
}

impl Fingerprint {
    // None if the file can't be examined, or isn't a regular file.
    pub fn of(path: &str) -> Option<Fingerprint> {
        let meta = fs::metadata(path).ok()?;
        if !meta.is_file() {
            return None;
        }
        let modified_ms = meta
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |since_epoch| since_epoch.as_millis() as u64);
        Some(Fingerprint { size: meta.len(), modified_ms })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Entry {
    fingerprint: Fingerprint,
    duration: Option<f64>, // None = we tried, and the file can't be read
}

#[derive(Serialize, Deserialize, Default)]
pub struct Cache {
    entries: HashMap<String, Entry>,
}

fn cache_path(dir: &Path) -> PathBuf {
    dir.join("durations.json")
}

impl Cache {
    // A missing or unreadable cache is just an empty cache. It's disposable.
    pub fn load(dir: &Path) -> Cache {
        fs::read_to_string(cache_path(dir))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, dir: &Path) -> io::Result<()> {
        fs::create_dir_all(dir)?;
        let path = cache_path(dir);
        let tmp = path.with_extension("json.tmp");
        let json = serde_json::to_string(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &path)
    }

    // None             -> never measured, or the file has changed since: probe it.
    // Some(None)       -> measured before, and it's unreadable.
    // Some(Some(secs)) -> measured before, and here is the length.
    pub fn lookup(&self, path: &str, fingerprint: Fingerprint) -> Option<Option<f64>> {
        let entry = self.entries.get(path)?;
        (entry.fingerprint == fingerprint).then_some(entry.duration)
    }

    pub fn insert(&mut self, path: String, fingerprint: Fingerprint, duration: Option<f64>) {
        self.entries.insert(path, Entry { fingerprint, duration });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("mpv-cache-test-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn survives_a_save_and_load_and_notices_changed_files() {
        let dir = temp_dir();
        let fp = Fingerprint { size: 10, modified_ms: 5 };

        let mut cache = Cache::default();
        cache.insert("/v/a.mp4".into(), fp, Some(61.5));
        cache.insert("/v/b.mp4".into(), fp, None);
        cache.save(&dir).unwrap();

        let loaded = Cache::load(&dir);
        assert_eq!(loaded.lookup("/v/a.mp4", fp), Some(Some(61.5)));
        assert_eq!(loaded.lookup("/v/b.mp4", fp), Some(None));
        assert_eq!(loaded.lookup("/v/c.mp4", fp), None); // never seen

        let changed = Fingerprint { size: 11, ..fp };
        assert_eq!(loaded.lookup("/v/a.mp4", changed), None); // file changed

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_or_corrupt_cache_loads_as_empty() {
        let dir = temp_dir();
        let fp = Fingerprint { size: 1, modified_ms: 1 };
        assert_eq!(Cache::load(&dir).lookup("/v/a.mp4", fp), None); // no file at all

        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("durations.json"), "{{ definitely not json").unwrap();
        assert_eq!(Cache::load(&dir).lookup("/v/a.mp4", fp), None); // garbage

        let _ = fs::remove_dir_all(&dir);
    }
}
