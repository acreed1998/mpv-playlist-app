use std::collections::HashMap;

use serde::Serialize;
use tauri::State;

use super::sorting::gather_durations;
use crate::mpv::process;
use crate::playlist::{
    check,
    model::{Playlist, SortKey},
    scan, sort,
};
use crate::state::{MpvState, PlaylistState, ProbeState, SettingsState};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddResult {
    playlist: Playlist,
    added: usize,
    duplicates_skipped: usize,
    unreadable: usize,
}

#[tauri::command]
pub fn load_playlist(state: State<'_, PlaylistState>) -> Result<Playlist, String> {
    Ok(state.lock()?.clone())
}

#[tauri::command]
pub fn get_video_extensions(settings: State<'_, SettingsState>) -> Result<Vec<String>, String> {
    Ok(settings.get()?.extensions())
}

#[tauri::command(async)]
pub fn add_paths(
    paths: Vec<String>,
    app: tauri::AppHandle,
    state: State<'_, PlaylistState>,
    probe_state: State<'_, ProbeState>,
    settings_state: State<'_, SettingsState>,
) -> Result<AddResult, String> {
    // 1. Slow part: walk the filesystem. No lock held.
    let extensions = settings_state.get()?.extensions();
    let scanned = scan::collect_videos(&paths, &extensions);

    // 2. Briefly lock to learn the current sort and the existing paths.
    let (key, mut all_paths) = {
        let playlist = state.lock()?;
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
    let mut playlist = state.lock()?;
    let outcome = playlist.add_files(scanned.files);
    if let Some(lengths) = &lengths {
        sort::reapply_current(&mut playlist, &times, lengths);
    }
    state.save(&playlist)?;

    Ok(AddResult {
        playlist: playlist.clone(),
        added: outcome.added,
        duplicates_skipped: outcome.duplicates_skipped,
        unreadable: scanned.unreadable,
    })
}

#[tauri::command]
pub fn set_allow_duplicates(
    allow: bool,
    state: State<'_, PlaylistState>,
) -> Result<Playlist, String> {
    let mut playlist = state.lock()?;
    playlist.allow_duplicates = allow;
    state.save(&playlist)?;
    Ok(playlist.clone())
}

#[tauri::command]
pub fn move_track(
    track_id: String,
    to_index: usize,
    state: State<'_, PlaylistState>,
) -> Result<Playlist, String> {
    let mut playlist = state.lock()?;
    playlist.move_track(&track_id, to_index)?;
    state.save(&playlist)?;
    Ok(playlist.clone())
}

#[tauri::command]
pub fn remove_tracks(
    ids: Vec<String>,
    state: State<'_, PlaylistState>,
) -> Result<Playlist, String> {
    let mut playlist = state.lock()?;
    playlist.remove_tracks(&ids);
    state.save(&playlist)?;
    Ok(playlist.clone())
}

#[tauri::command(async)]
pub fn clear_playlist(
    playlist_state: State<'_, PlaylistState>,
    mpv_state: State<'_, MpvState>,
) -> Result<Playlist, String> {
    // Stop mpv FIRST. This also retires the observer, so it can't write a
    // resume point for a track we're about to delete.
    process::halt_mpv(&mpv_state, &playlist_state);

    let mut playlist = playlist_state.lock()?;
    playlist.clear();
    playlist_state.save(&playlist)?;
    Ok(playlist.clone())
}

#[tauri::command(async)]
pub fn find_missing_tracks(state: State<'_, PlaylistState>) -> Result<Vec<String>, String> {
    // Copy the tracks under a brief lock, then check the disk with no lock held.
    let tracks = {
        let playlist = state.lock()?;
        playlist.tracks.clone()
    };
    Ok(check::find_missing(&tracks))
}
