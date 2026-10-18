use std::path::PathBuf;
use std::process::Child;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Mutex, MutexGuard};

use crate::durations::Cache;
use crate::playlist::{model::Playlist, storage};

pub const DEFAULT_PLAYLIST_ID: &str = "default";

// The mpv process we started (if any), plus the counter that retires old observers.
#[derive(Default)]
pub struct MpvState {
    pub child: Mutex<Option<Child>>,
    pub generation: AtomicU64, // bumped every time mpv is stopped or replaced
}

// The one in-memory copy of the playlist, plus where to save it.
pub struct PlaylistState {
    pub dir: PathBuf,
    pub playlist: Mutex<Playlist>,
}

// The duration cache, held for the whole duration of a probe job.
pub struct ProbeState {
    pub dir: PathBuf,
    pub cache: Mutex<Cache>,
    pub cancel: AtomicBool,
}

impl PlaylistState {
    // Lock the playlist, turning a "poisoned" error into a plain string.
    pub fn lock(&self) -> Result<MutexGuard<'_, Playlist>, String> {
        self.playlist
            .lock()
            .map_err(|_| "playlist lock was poisoned".to_string())
    }

    // Write the playlist to disk, with a friendly error message.
    pub fn save(&self, playlist: &Playlist) -> Result<(), String> {
        storage::save(&self.dir, playlist).map_err(|e| format!("Could not save playlist: {e}"))
    }
}
