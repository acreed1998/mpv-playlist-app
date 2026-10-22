use std::path::PathBuf;
use std::process::Child;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Mutex, MutexGuard};

use crate::durations::Cache;
use crate::playlist::{model::Playlist, storage};
use crate::settings::{self, Settings};

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

// User settings, plus where they're saved.
pub struct SettingsState {
    pub dir: PathBuf,
    pub settings: Mutex<Settings>,
}

impl SettingsState {
    pub fn get(&self) -> Result<Settings, String> {
        self.settings
            .lock()
            .map(|s| s.clone())
            .map_err(|_| "settings lock was poisoned".to_string())
    }

    // The mpv program to run. Falls back to the default if the lock is
    // poisoned, because failing to launch over a settings glitch is worse.
    pub fn mpv_program(&self) -> String {
        self.get().unwrap_or_default().mpv_program()
    }

    // Change settings the safe way: copy, modify the copy, write it to disk,
    // and only then replace the in-memory value. If the write fails, memory
    // and disk still agree.
    fn update(&self, change: impl FnOnce(&mut Settings)) -> Result<Settings, String> {
        let mut current = self
            .settings
            .lock()
            .map_err(|_| "settings lock was poisoned".to_string())?;

        let mut updated = current.clone();
        change(&mut updated);
        settings::save(&self.dir, &updated)
            .map_err(|e| format!("Could not save settings: {e}"))?;
        *current = updated.clone();
        Ok(updated)
    }

    pub fn set_mpv_path(&self, path: Option<String>) -> Result<Settings, String> {
        self.update(|s| s.mpv_path = path)
    }

    pub fn set_video_extensions(&self, list: Option<Vec<String>>) -> Result<Settings, String> {
        self.update(|s| s.video_extensions = list)
    }

    pub fn set_playback_speed(&self, speed: Option<String>) -> Result<Settings, String> {
        self.update(|s| s.playback_speed = speed)
    }

    pub fn set_restart_mpv_on_track_end(&self, enabled: bool) -> Result<Settings, String> {
        self.update(|s| s.restart_mpv_on_track_end = enabled)
    }
}
