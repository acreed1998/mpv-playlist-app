mod commands;
mod dependencies;
mod durations;
mod mpv;
mod playlist;
mod state;
mod settings;

use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use tauri::Manager;

use durations::Cache;
use playlist::storage;
use state::{MpvState, PlaylistState, ProbeState, SettingsState, DEFAULT_PLAYLIST_ID};

// Runs once, before the window opens: load saved data and register shared state.
fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let dir = app.path().app_data_dir()?;
    let playlist = storage::load_or_create(&dir, DEFAULT_PLAYLIST_ID, "My Playlist")?;

    app.manage(ProbeState {
        cache: Mutex::new(Cache::load(&dir)),
        dir: dir.clone(),
        cancel: AtomicBool::new(false),
    });
    app.manage(SettingsState {
        settings: Mutex::new(settings::load(&dir)),
        dir: dir.clone(),
    });
    app.manage(PlaylistState {
        dir,
        playlist: Mutex::new(playlist),
    });
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(MpvState::default())
        .setup(setup)
        .invoke_handler(tauri::generate_handler![
            dependencies::check_dependencies,
            commands::playback::play_playlist,
            commands::playback::stop_video,
            commands::playlist::load_playlist,
            commands::playlist::get_video_extensions,
            commands::playlist::add_paths,
            commands::playlist::set_allow_duplicates,
            commands::playlist::move_track,
            commands::playlist::remove_tracks,
            commands::playlist::clear_playlist,
            commands::playlist::find_missing_tracks,
            commands::sorting::sort_playlist,
            commands::sorting::sort_by_length,
            commands::sorting::cancel_probe,
            commands::settings::get_settings,
            commands::settings::set_mpv_path,
            commands::settings::set_video_extensions,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            mpv::process::shutdown(app_handle);
        }
    });
}
