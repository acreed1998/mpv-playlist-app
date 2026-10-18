use std::sync::TryLockError;
use std::sync::atomic::Ordering;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::durations::{self, Outcome};
use crate::playlist::model::{Direction, Playlist, SortKey, SortSettings};
use crate::playlist::sort;
use crate::state::{PlaylistState, ProbeState};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProbeProgress {
    done: usize,
    total: usize,
}

// Get lengths for every path (probing what the cache can't answer), reporting
// progress to the UI as it goes. Also used by `add_paths` in playlist.rs.
pub(super) fn gather_durations(
    app: &AppHandle,
    probe_state: &ProbeState,
    paths: &[String],
) -> Result<Outcome, String> {
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
pub fn sort_playlist(
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

    // 1. Briefly lock just to copy the paths.
    let paths: Vec<String> = {
        let playlist = state.lock()?;
        playlist.tracks.iter().map(|t| t.path.clone()).collect()
    };

    // 2. Slow part, no lock held: ask the filesystem for dates (date sorts only).
    let times = sort::read_times(&paths, key);

    // 3. Lock again, compute the new order, save.
    let mut playlist = state.lock()?;
    let seed = sort::new_seed();
    let order = sort::sorted_order(&playlist.tracks, key, direction, seed, &times);

    playlist.order = order;
    playlist.sort = SortSettings {
        key,
        direction,
        seed: matches!(key, SortKey::Random | SortKey::ShuffleFolders).then_some(seed),
    };

    state.save(&playlist)?;
    Ok(playlist.clone())
}

#[tauri::command(async)]
pub fn sort_by_length(
    direction: Direction,
    app: AppHandle,
    playlist_state: State<'_, PlaylistState>,
    probe_state: State<'_, ProbeState>,
) -> Result<Option<Playlist>, String> {
    // 1. Briefly lock to copy the paths.
    let paths: Vec<String> = {
        let playlist = playlist_state.lock()?;
        playlist.tracks.iter().map(|t| t.path.clone()).collect()
    };

    // 2. The slow part, with no playlist lock held.
    let found = gather_durations(&app, &probe_state, &paths)?;
    if found.cancelled {
        return Ok(None); // the user backed out: change nothing
    }

    // 3. Lock again, apply, save.
    let mut playlist = playlist_state.lock()?;
    let order = sort::length_order(&playlist.tracks, direction, &found.durations);
    playlist.order = order;
    playlist.sort = SortSettings {
        key: SortKey::Length,
        direction,
        seed: None,
    };
    playlist_state.save(&playlist)?;
    Ok(Some(playlist.clone()))
}

#[tauri::command]
pub fn cancel_probe(state: State<'_, ProbeState>) {
    state.cancel.store(true, Ordering::SeqCst);
}
