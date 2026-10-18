use std::io;
use std::process::Command;
use serde::Serialize;

use tauri::State;
use crate::state::SettingsState;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

// Build a Command that won't pop up a console window on Windows.
// pub(crate) means "visible to the rest of this app, but not to outside code".
pub(crate) fn quiet_command(program: &str) -> Command {
    #[allow(unused_mut)] // `cmd` only needs to be mutable on Windows
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    cmd
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    pub name: String,
    pub available: bool,
    pub version: Option<String>, // first line of the tool's version output
    pub error: Option<String>,   // why it's unavailable, in plain language
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DependencyReport {
    pub mpv: ToolStatus,
    pub ffprobe: ToolStatus,
}

fn unavailable(name: &str, error: String) -> ToolStatus {
    ToolStatus {
        name: name.to_string(),
        available: false,
        version: None,
        error: Some(error),
    }
}

// Try to run `<program> <version_flag>` and describe what happened.
// `name` is what we call the tool in the UI; `program` is what we actually run
// (the same, unless the user configured a path).
fn check_tool(name: &str, program: &str, version_flag: &str) -> ToolStatus {
    match quiet_command(program).arg(version_flag).output() {
        // It ran and exited successfully: grab the first line of output.
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let version = stdout.lines().next().map(|line| line.trim().to_string());
            ToolStatus {
                name: name.to_string(),
                available: true,
                version,
                error: None,
            }
        }
        // It ran, but reported failure.
        Ok(output) => unavailable(name, format!("{name} ran but exited with {}", output.status)),
        // It couldn't be started at all. NotFound is the "not installed" case.
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let place = if program == name {
                "on your PATH".to_string()
            } else {
                format!("at {program}")
            };
            unavailable(name, format!("{name} was not found {place}"))
        }
        Err(e) => unavailable(name, format!("could not run {program}: {e}")),
    }
}

#[tauri::command(async)]
pub fn check_dependencies(settings: State<'_, SettingsState>) -> DependencyReport {
    DependencyReport {
        mpv: check_tool("mpv", &settings.mpv_program(), "--version"),
        ffprobe: check_tool("ffprobe", "ffprobe", "-version"), // ffprobe uses a single dash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_program_is_reported_as_not_found() {
        let status = check_tool("definitely-not-a-real-program-xyz", "definitely-not-a-real-program-xyz", "--version");
        assert!(!status.available);
        assert!(status.error.unwrap().contains("not found"));
    }

    #[test]
    fn existing_program_reports_its_version() {
        // `cargo test` is running under cargo, so cargo is guaranteed to exist.
        let status = check_tool("cargo", "cargo", "--version");
        assert!(status.available);
        assert!(status.version.unwrap().starts_with("cargo"));
    }
}
