use std::fs;
use std::sync::atomic::Ordering;

use tauri::{Emitter, State};

use crate::mpv::{observer, process};
use crate::playlist::queue;
use crate::state::{MpvState, PlaylistState, SettingsState};

#[tauri::command(async)]
pub fn play_playlist(
    start_track_id: Option<String>,
    resume: bool,
    app: tauri::AppHandle,
    playlist_state: State<'_, PlaylistState>,
    mpv_state: State<'_, MpvState>,
    settings_state: State<'_, SettingsState>,
) -> Result<(), String> {
    // 1. Snapshot the queue and decide where to start. The lock ends at the closing brace.
    let (queue, start_secs) = {
        let playlist = playlist_state.lock()?;
        let start = queue::resolve_start(&playlist, start_track_id.as_deref(), resume);
        let queue = queue::build_queue(&playlist, start.track_id.as_deref())?;
        (queue, start.seconds)
    };

    // 2. Write the queue file where mpv can read it in normal playlist mode.
    let queue_path = playlist_state.dir.join("queue.m3u8");
    let settings = settings_state.get()?;
    if !settings.restart_mpv_on_track_end {
        fs::write(&queue_path, queue::queue_file_text(&queue.paths))
            .map_err(|e| format!("Could not write the queue file: {e}"))?;
    }

    // 3. Stop the previous mpv (saving its position) and retire its observer.
    process::halt_mpv(&mpv_state, &playlist_state);

    // 4. Start the new mpv, using the configured program.
    let program = settings.mpv_program();
    let playback_speed = settings.mpv_playback_speed();
    let mut slot = mpv_state
        .child
        .lock()
        .map_err(|_| "mpv state lock was poisoned".to_string())?;
    process::stop_existing(&mut slot); // normally already empty
    *slot = Some(if settings.restart_mpv_on_track_end {
        process::spawn_mpv_track(&program, &queue.paths[queue.start_index], &playback_speed)?
    } else {
        process::spawn_mpv(&program, &queue_path, queue.start_index, &playback_speed)?
    });

    // 5. Clear any old tracking warning, then start watching the new session.
    let _ = app.emit("tracking-status", None::<String>);
    let generation = mpv_state.generation.fetch_add(1, Ordering::SeqCst) + 1;
    observer::spawn(
        app,
        generation,
        observer::Session {
            track_ids: queue.track_ids,
            paths: queue.paths,
            start_index: queue.start_index,
            start_secs,
            restart_mpv_on_track_end: settings.restart_mpv_on_track_end,
            program,
            playback_speed,
        },
    );

    Ok(())
}

#[tauri::command]
pub fn stop_video(
    mpv_state: State<'_, MpvState>,
    playlist_state: State<'_, PlaylistState>,
) -> Result<(), String> {
    process::halt_mpv(&mpv_state, &playlist_state);
    Ok(())
}
