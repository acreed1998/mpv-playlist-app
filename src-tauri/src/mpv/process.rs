use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::Ordering;

use tauri::Manager;

use super::ipc;
use crate::state::{MpvState, PlaylistState};
use crate::dependencies::quiet_command;

// Kill and clean up the process in the slot, if there is one.
pub fn stop_existing(slot: &mut Option<Child>) {
    if let Some(mut child) = slot.take() {
        let _ = child.kill(); // ignore errors: it may have already exited
        let _ = child.wait(); // reap it so it doesn't linger as a zombie
    }
}

// Launch mpv on a queue file, starting at the given entry.
// Everything about HOW mpv is started lives here, so there is one place to change.
pub fn spawn_mpv(program: &str, queue_path: &Path, start_index: usize, playback_speed: &str) -> Result<Child, String> {
    // A killed mpv can't delete its socket; remove it so we only ever
    // connect to the NEW mpv.
    ipc::remove_stale_socket();

    quiet_command(program)
        .arg(format!("--input-ipc-server={}", ipc::IPC_PATH))
        .arg(format!("--playlist-start={start_index}"))
        .arg(format!("--playlist={}", queue_path.display()))
        .arg(format!("--speed={}", playback_speed))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("Failed to start {program}: {e}"))
}

// Stop mpv for good (or before replacing it), saving the position first.
// The two locks are taken one after the other, never nested.
pub fn halt_mpv(mpv: &MpvState, playlist_state: &PlaylistState) {
    // 1. Invalidate the observer: from here on, it can't write anything.
    mpv.generation.fetch_add(1, Ordering::SeqCst);

    // 2. Write the playlist (including the latest position) to disk.
    if let Ok(playlist) = playlist_state.lock() {
        let _ = playlist_state.save(&playlist);
    }

    // 3. Now it's safe to kill mpv.
    if let Ok(mut slot) = mpv.child.lock() {
        stop_existing(&mut slot);
    }
}

// Called when the app exits, for whatever reason it exits.
pub fn shutdown(app: &tauri::AppHandle) {
    if let (Some(mpv), Some(playlist)) = (
        app.try_state::<MpvState>(),
        app.try_state::<PlaylistState>(),
    ) {
        halt_mpv(&mpv, &playlist);
    }
}
