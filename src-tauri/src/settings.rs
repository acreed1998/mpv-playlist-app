use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use crate::playlist::scan;

const DEFAULT_MPV: &str = "mpv";

// `default` means: any field missing from the file gets its default value.
// Files written by older versions of the app keep loading as settings are added.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub mpv_path: Option<String>, // None = look for "mpv" on the PATH
    pub video_extensions: Option<Vec<String>>, // None = the built-in list
}

impl Settings {
    // The program name or path to run. The only place that default is decided.
    pub fn mpv_program(&self) -> String {
        self.mpv_path
            .clone()
            .unwrap_or_else(|| DEFAULT_MPV.to_string())
    }

    // The extensions in effect right now. The only place that default is decided.
    pub fn extensions(&self) -> Vec<String> {
        self.video_extensions
            .clone()
            .unwrap_or_else(scan::default_extensions)
    }
}

fn settings_path(dir: &Path) -> PathBuf {
    dir.join("settings.json")
}

// A missing or unreadable settings file just means "defaults".
pub fn load(dir: &Path) -> Settings {
    fs::read_to_string(settings_path(dir))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save(dir: &Path, settings: &Settings) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let path = settings_path(dir);
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(&tmp, json)?;
    fs::rename(&tmp, &path)
}

// Clean up what the user typed or pasted. Empty means "use the default".
pub fn normalize_path(raw: &str) -> Option<String> {
    let cleaned = raw
        .trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .trim();
    (!cleaned.is_empty()).then(|| cleaned.to_string())
}

// Turn what the user typed ("MP4, .mkv  *.avi") into a clean list.
// Entries may be separated by commas, semicolons, or whitespace.
// An empty result is not an error; the caller treats it as "use the defaults".
pub fn parse_extensions(raw: &str) -> Result<Vec<String>, String> {
    let mut result: Vec<String> = Vec::new();
    let mut invalid: Vec<String> = Vec::new();

    let tokens = raw
        .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .filter(|token| !token.is_empty());

    for token in tokens {
        // Accept ".mp4" and "*.mp4" as well as "mp4".
        let ext = token
            .trim_start_matches(|c| c == '*' || c == '.')
            .to_ascii_lowercase();

        if ext.is_empty() || !ext.chars().all(|c| c.is_ascii_alphanumeric()) {
            invalid.push(token.to_string());
        } else if !result.contains(&ext) {
            result.push(ext); // keeps the user's order, skips repeats
        }
    }

    if invalid.is_empty() {
        Ok(result)
    } else {
        Err(format!(
            "These aren't valid video extensions: {}. Use letters and digits only, separated by commas (for example: mkv).",
            invalid.join(", ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("mpv-settings-test-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn survives_a_save_and_load() {
        let dir = temp_dir();
        let settings = Settings { mpv_path: Some("/opt/mpv/bin/mpv".into()), ..Settings::default() };

        save(&dir, &settings).unwrap();
        assert_eq!(load(&dir), settings);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_corrupt_or_empty_files_load_as_defaults() {
        let dir = temp_dir();
        assert_eq!(load(&dir), Settings::default()); // no file at all

        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("settings.json"), "{{ not json").unwrap();
        assert_eq!(load(&dir), Settings::default()); // garbage

        fs::write(dir.join("settings.json"), "{}").unwrap();
        assert_eq!(load(&dir), Settings::default()); // valid, but no fields

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn program_defaults_to_mpv_on_the_path() {
        assert_eq!(Settings::default().mpv_program(), "mpv");
        let custom = Settings { mpv_path: Some("/x/mpv".into()), ..Settings::default() };
        assert_eq!(custom.mpv_program(), "/x/mpv");
    }

    #[test]
    fn pasted_paths_are_cleaned_up() {
        assert_eq!(normalize_path("  /usr/bin/mpv \n"), Some("/usr/bin/mpv".into()));
        assert_eq!(
            normalize_path(r#""C:\Tools\mpv\mpv.exe""#),
            Some(r"C:\Tools\mpv\mpv.exe".into())
        );
        assert_eq!(normalize_path("   "), None);
        assert_eq!(normalize_path(r#""""#), None);
    }

    #[test]
    fn typed_extensions_are_cleaned_and_deduplicated() {
        assert_eq!(
            parse_extensions("MP4, .mkv  *.avi;webm,mp4").unwrap(),
            vec!["mp4", "mkv", "avi", "webm"]
        );
        assert!(parse_extensions("  ,, ").unwrap().is_empty());
    }

    #[test]
    fn invalid_extensions_are_reported_together() {
        let err = parse_extensions("mp4, m$v, tar.gz, .").unwrap_err();
        assert!(err.contains("m$v"));
        assert!(err.contains("tar.gz"));
    }

    #[test]
    fn extensions_fall_back_to_the_defaults() {
        assert!(Settings::default().extensions().contains(&"mkv".to_string()));

        let custom = Settings { video_extensions: Some(vec!["ts".into()]), ..Settings::default() };
        assert_eq!(custom.extensions(), vec!["ts"]);
    }
}
