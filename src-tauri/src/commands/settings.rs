use tauri::State;

use crate::settings::{self, Settings};
use crate::state::SettingsState;

#[tauri::command]
pub fn get_settings(state: State<'_, SettingsState>) -> Result<Settings, String> {
    state.get()
}

#[tauri::command]
pub fn set_mpv_path(
    path: Option<String>,
    state: State<'_, SettingsState>,
) -> Result<Settings, String> {
    // `None`, an empty string, and "only quotes" all mean "use the default".
    let cleaned = path.as_deref().and_then(settings::normalize_path);
    state.set_mpv_path(cleaned)
}

#[tauri::command]
pub fn set_video_extensions(
    text: Option<String>,
    state: State<'_, SettingsState>,
) -> Result<Settings, String> {
    // Nothing typed (or only separators) means "use the built-in list".
    let list = match text.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(raw) => {
            let parsed = settings::parse_extensions(raw)?;
            (!parsed.is_empty()).then_some(parsed)
        }
    };
    state.set_video_extensions(list)
}

#[tauri::command]
pub fn set_playback_speed(
    speed: Option<String>,
    state: State<'_, SettingsState>,
) -> Result<Settings, String> {
    // `None`, an empty string, and "only quotes" all mean "use the default".
    let cleaned = speed.as_deref().and_then(settings::normalize_path);
    state.set_playback_speed(cleaned)
}
