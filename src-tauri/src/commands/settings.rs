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
