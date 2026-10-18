use std::io;

use serde_json::Value;

use crate::dependencies::quiet_command;

// ffprobe prints: {"format": {"duration": "125.480000"}}
// The duration is a STRING, and for some files it's missing or "N/A".
pub fn parse_duration(json_text: &str) -> Option<f64> {
    let value: Value = serde_json::from_str(json_text).ok()?;
    let secs: f64 = value["format"]["duration"].as_str()?.parse().ok()?;
    // "nan" and "inf" parse successfully as floats, so reject them explicitly.
    (secs.is_finite() && secs >= 0.0).then_some(secs)
}

// Ok(Some(secs)): the length.
// Ok(None):       ffprobe ran but couldn't read the file (corrupt, not media, ...).
// Err(message):   ffprobe itself couldn't be run, which says nothing about this file.
pub fn probe_file(path: &str) -> Result<Option<f64>, String> {
    let result = quiet_command("ffprobe")
        .args([
            "-v", "error",
            "-show_entries", "format=duration",
            "-of", "json",
            "-i", path,
        ])
        .output();

    match result {
        Ok(output) if output.status.success() => {
            Ok(parse_duration(&String::from_utf8_lossy(&output.stdout)))
        }
        Ok(_) => Ok(None),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            Err("ffprobe was not found on your PATH".to_string())
        }
        Err(e) => Err(format!("could not run ffprobe: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_duration_string() {
        assert_eq!(parse_duration(r#"{"format":{"duration":"125.480000"}}"#), Some(125.48));
    }

    #[test]
    fn anything_unusable_is_none() {
        assert_eq!(parse_duration(r#"{"format":{}}"#), None);
        assert_eq!(parse_duration("{}"), None);
        assert_eq!(parse_duration(r#"{"format":{"duration":"N/A"}}"#), None);
        assert_eq!(parse_duration(r#"{"format":{"duration":"-5"}}"#), None);
        assert_eq!(parse_duration(r#"{"format":{"duration":"nan"}}"#), None);
        assert_eq!(parse_duration("not json"), None);
    }
}
