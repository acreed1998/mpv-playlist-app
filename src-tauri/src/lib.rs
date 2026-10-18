mod dependencies;
mod mpv;
mod playlist;
mod durations;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use std::fs;
use std::collections::HashMap;
use playlist::{
    check,
    model::{Direction, Playlist, SortKey, SortSettings},
    queue, scan, sort, storage,
};
use std::process::{Child, Command, Stdio};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::TryLockError;
use serde::Serialize;
use tauri::{Emitter, Manager, State};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use mpv::{ipc, observer};
use durations::Cache;

// Shared state: "the mpv process we started, if any".
#[derive(Default)]
struct MpvState {
    child: Mutex<Option<Child>>,
    generation: AtomicU64, // bumped every time mpv is stopped or replaced
}

// Kill and clean up the process in the slot, if there is one
fn stop_existing(slot: &mut Option<Child>) {
    if let Some(mut child) = slot.take() {
        let _ = child.kill(); // ignore errors: it may have already existed
        let _ = child.wait(); // reap it so it doesn't linger as a zombie
    }
}

// Stop mpv for good (or before replacing it), saving the position first.
// The two locks are taken one after the other, never nested.
fn halt_mpv(mpv: &MpvState, playlist: &PlaylistState) {
    // 1. Invalidate the observer: from here on, it can't write anything.
    mpv.generation.fetch_add(1, Ordering::SeqCst);

    // 2. Write the playlist (including the latest position) to disk.
    if let Ok(playlist_data) = playlist.playlist.lock() {
        let _ = storage::save(&playlist.dir, &playlist_data);
    }

    // 3. Now it's safe to kill mpv.
    if let Ok(mut slot) = mpv.child.lock() {
        stop_existing(&mut slot);
    }
}

// Called when the app exits, for whatever reason it exits.
fn shutdown(app: &tauri::AppHandle) {
    if let (Some(mpv), Some(playlist)) =
        (app.try_state::<MpvState>(), app.try_state::<PlaylistState>())
    {
        halt_mpv(&mpv, &playlist);
    }
}

#[tauri::command(async)]
fn play_playlist(
    start_track_id: Option<String>,
    resume: bool,
    app: tauri::AppHandle,
    playlist_state: State<'_, PlaylistState>,
    mpv_state: State<'_, MpvState>,
) -> Result<(), String> {
    // 1. Snapshot the queue and decide where to start. The lock ends at the closing brace.
    let (queue, start_secs) = {
        let playlist = lock_playlist(&playlist_state)?;
        let start = queue::resolve_start(&playlist, start_track_id.as_deref(), resume);
        let queue = queue::build_queue(&playlist, start.track_id.as_deref())?;
        (queue, start.seconds)
    };

    // 2. Write the queue file where mpv can read it.
    let queue_path = playlist_state.dir.join("queue.m3u8");
    fs::write(&queue_path, queue::queue_file_text(&queue.paths))
        .map_err(|e| format!("Could not write the queue file: {e}"))?;

    // 3. Stop the previous mpv (saving its position) and retire its observer.
    halt_mpv(&mpv_state, &playlist_state);

    // 4. Start the new mpv.
    let mut slot = mpv_state
        .child
        .lock()
        .map_err(|_| "mpv state lock was poisoned".to_string())?;
    stop_existing(&mut slot); // normally already empty

    ipc::remove_stale_socket();
    let child = Command::new("mpv")
        .arg(format!("--input-ipc-server={}", ipc::IPC_PATH))
        .arg(format!("--playlist-start={}", queue.start_index))
        .arg(format!("--playlist={}", queue_path.display()))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("Failed to start mpv: {e}"))?;
    *slot = Some(child);

    // 5. Start watching it. This launch gets its own generation number.
    let generation = mpv_state.generation.fetch_add(1, Ordering::SeqCst) + 1;
    observer::spawn(
        app,
        generation,
        observer::Session {
            track_ids: queue.track_ids,
            start_index: queue.start_index,
            start_secs,
        },
    );

    Ok(())
}

#[tauri::command]
fn stop_video(
    mpv_state: State<'_, MpvState>,
    playlist_state: State<'_, PlaylistState>,
) -> Result<(), String> {
    halt_mpv(&mpv_state, &playlist_state);
    Ok(())
}

const DEFAULT_PLAYLIST_ID: &str = "default";

// The one in-memory copy of the playlist, plus where to save it.
struct PlaylistState {
    dir: PathBuf,
    playlist: Mutex<Playlist>,
}

struct ProbeState {
    dir: PathBuf,
    cache: Mutex<Cache>, // held for the whole duration of a probe job
    cancel: AtomicBool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProbeProgress {
    done: usize,
    total: usize,
}

// Lock the playlist, converting a "poisoned" error into a plain string.
fn lock_playlist<'a>(
    state: &'a PlaylistState,
) -> Result<std::sync::MutexGuard<'a, Playlist>, String> {
    state
        .playlist
        .lock()
        .map_err(|_| "playlist lock was poisoned".to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AddResult {
    playlist: Playlist,
    added: usize,
    duplicates_skipped: usize,
    unreadable: usize,
}

#[tauri::command]
fn load_playlist(state: State<'_, PlaylistState>) -> Result<Playlist, String> {
    Ok(lock_playlist(&state)?.clone())
}

#[tauri::command(async)]
fn sort_playlist(
    key: SortKey,
    direction: Direction,
    state: State<'_, PlaylistState>,
) -> Result<Playlist, String> {
    if key == SortKey::Custom {
        return Err("\"Custom\" is set by dragging tracks; it can't be chosen as a sort.".into());
    }
    if key == SortKey::Length {
        return Err("Length sorting uses its own command (sort_by_length).".into());
    }

    // 1. Briefly lock just to copy the paths. The extra { } block makes the
    //    guard drop at the closing brace, so the lock is released right away.
    let paths: Vec<String> = {
        let playlist = lock_playlist(&state)?;
        playlist.tracks.iter().map(|t| t.path.clone()).collect()
    };

    // 2. Slow part, no lock held: ask the filesystem for dates (date sorts only).
    let times = sort::read_times(&paths, key);

    // 3. Lock again, compute the new order, save.
    let mut playlist = lock_playlist(&state)?;
    let seed = sort::new_seed();
    let order = sort::sorted_order(&playlist.tracks, key, direction, seed, &times);

    playlist.order = order;
    playlist.sort = SortSettings {
        key,
        direction,
        seed: matches!(key, SortKey::Random | SortKey::ShuffleFolders).then_some(seed),
    };

    storage::save(&state.dir, &playlist).map_err(|e| format!("Could not save playlist: {e}"))?;
    Ok(playlist.clone())
}

#[tauri::command]
fn remove_tracks(ids: Vec<String>, state: State<'_, PlaylistState>) -> Result<Playlist, String> {
    let mut playlist = lock_playlist(&state)?;
    playlist.remove_tracks(&ids);
    storage::save(&state.dir, &playlist).map_err(|e| format!("Could not save playlist: {e}"))?;
    Ok(playlist.clone())
}

#[tauri::command(async)]
fn clear_playlist(
    playlist_state: State<'_, PlaylistState>,
    mpv_state: State<'_, MpvState>,
) -> Result<Playlist, String> {
    // Stop mpv FIRST. This also retires the observer, so it can't write a
    // resume point for a track we're about to delete.
    halt_mpv(&mpv_state, &playlist_state);

    let mut playlist = lock_playlist(&playlist_state)?;
    playlist.clear();
    storage::save(&playlist_state.dir, &playlist)
        .map_err(|e| format!("Could not save playlist: {e}"))?;
    Ok(playlist.clone())
}

#[tauri::command(async)]
fn find_missing_tracks(state: State<'_, PlaylistState>) -> Result<Vec<String>, String> {
    // Copy the tracks under a brief lock, then check the disk with no lock held.
    let tracks = {
        let playlist = lock_playlist(&state)?;
        playlist.tracks.clone()
    };
    Ok(check::find_missing(&tracks))
}

#[tauri::command]
fn get_video_extensions() -> Vec<&'static str> {
    scan::VIDEO_EXTENSIONS.to_vec()
}

#[tauri::command(async)]
fn add_paths(
    paths: Vec<String>,
    app: tauri::AppHandle,
    state: State<'_, PlaylistState>,
    probe_state: State<'_, ProbeState>,
) -> Result<AddResult, String> {
    // 1. Slow part: walk the filesystem. No lock held.
    let scanned = scan::collect_videos(&paths);

    // 2. Briefly lock to learn the current sort and the existing paths.
    let (key, mut all_paths) = {
        let playlist = lock_playlist(&state)?;
        let existing: Vec<String> = playlist.tracks.iter().map(|t| t.path.clone()).collect();
        (playlist.sort.key, existing)
    };
    all_paths.extend(scanned.files.iter().cloned());

    // 3. Slow parts again, no lock: file dates and/or video lengths, if the sort needs them.
    let times = sort::read_times(&all_paths, key);

    // `None` means "we couldn't get every length", and then the new files are
    // just appended instead of re-sorting.
    let lengths: Option<HashMap<String, f64>> = match key {
        SortKey::Length => match gather_durations(&app, &probe_state, &all_paths) {
            Ok(found) if !found.cancelled => Some(found.durations),
            _ => None,
        },
        _ => Some(HashMap::new()),
    };

    // 4. Lock, merge, re-apply the sort, save.
    let mut playlist = lock_playlist(&state)?;
    let outcome = playlist.add_files(scanned.files);
    if let Some(lengths) = &lengths {
        sort::reapply_current(&mut playlist, &times, lengths);
    }
    storage::save(&state.dir, &playlist)
        .map_err(|e| format!("Could not save playlist: {e}"))?;

    Ok(AddResult {
        playlist: playlist.clone(),
        added: outcome.added,
        duplicates_skipped: outcome.duplicates_skipped,
        unreadable: scanned.unreadable,
    })
}

#[tauri::command]
fn set_allow_duplicates(allow: bool, state: State<'_, PlaylistState>) -> Result<Playlist, String> {
    let mut playlist = lock_playlist(&state)?;
    playlist.allow_duplicates = allow;
    storage::save(&state.dir, &playlist)
        .map_err(|e| format!("Could not save playlist: {e}"))?;
    Ok(playlist.clone())
}

#[tauri::command]
fn move_track(
    track_id: String,
    to_index: usize,
    state: State<'_, PlaylistState>,
) -> Result<Playlist, String> {
    let mut playlist = lock_playlist(&state)?;
    playlist.move_track(&track_id, to_index)?;
    storage::save(&state.dir, &playlist).map_err(|e| format!("Could not save playlist: {e}"))?;
    Ok(playlist.clone())
}

// Get lengths for every path (probing what the cache can't answer), reporting
// progress to the UI as it goes.
fn gather_durations(
    app: &tauri::AppHandle,
    probe_state: &ProbeState,
    paths: &[String],
) -> Result<durations::Outcome, String> {
    // try_lock: if another job holds the cache, fail now rather than wait.
    let mut cache = match probe_state.cache.try_lock() {
        Ok(guard) => guard,
        Err(TryLockError::WouldBlock) => {
            return Err("Another length scan is already running.".to_string());
        }
        Err(TryLockError::Poisoned(_)) => {
            return Err("duration cache lock was poisoned".to_string());
        }
    };
    probe_state.cancel.store(false, Ordering::SeqCst);

    let result = durations::ensure_durations(
        paths,
        &mut cache,
        &probe_state.cancel,
        durations::probe::probe_file,
        |done, total| {
            let _ = app.emit("probe-progress", ProbeProgress { done, total });
        },
    );

    // Save even if the job failed or was cancelled, so finished work isn't lost.
    let _ = cache.save(&probe_state.dir);
    result
}

#[tauri::command(async)]
fn sort_by_length(
    direction: Direction,
    app: tauri::AppHandle,
    playlist_state: State<'_, PlaylistState>,
    probe_state: State<'_, ProbeState>,
) -> Result<Option<Playlist>, String> {
    // 1. Briefly lock to copy the paths.
    let paths: Vec<String> = {
        let playlist = lock_playlist(&playlist_state)?;
        playlist.tracks.iter().map(|t| t.path.clone()).collect()
    };

    // 2. The slow part, with no playlist lock held.
    let found = gather_durations(&app, &probe_state, &paths)?;
    if found.cancelled {
        return Ok(None); // the user backed out: change nothing
    }

    // 3. Lock again, apply, save.
    let mut playlist = lock_playlist(&playlist_state)?;
    let order = sort::length_order(&playlist.tracks, direction, &found.durations);
    playlist.order = order;
    playlist.sort = SortSettings { key: SortKey::Length, direction, seed: None };
    storage::save(&playlist_state.dir, &playlist)
        .map_err(|e| format!("Could not save playlist: {e}"))?;
    Ok(Some(playlist.clone()))
}

#[tauri::command]
fn cancel_probe(state: State<'_, ProbeState>) {
    state.cancel.store(true, Ordering::SeqCst);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(MpvState::default())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;

            app.manage(ProbeState {
                cache: Mutex::new(Cache::load(&dir)),
                dir: dir.clone(),
                cancel: AtomicBool::new(false),
            });

            let playlist =
                storage::load_or_create(&dir, DEFAULT_PLAYLIST_ID, "My Playlist")?;
            app.manage(PlaylistState {
                dir,
                playlist: Mutex::new(playlist),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            dependencies::check_dependencies,
            play_playlist,
            stop_video,
            load_playlist,
            sort_playlist,
            remove_tracks,
            clear_playlist,
            find_missing_tracks,
            get_video_extensions,
            add_paths,
            set_allow_duplicates,
            move_track,
            sort_by_length,
            cancel_probe
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            shutdown(app_handle);
        }
    });
}
