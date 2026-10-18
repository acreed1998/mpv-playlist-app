use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::model::Playlist;

fn playlist_path(dir: &Path, id: &str) -> PathBuf {
    dir.join("playlists").join(format!("{id}.json"))
}

// Ok(None) means "no saved playlist yet", which isn't an error.
pub fn load(dir: &Path, id: &str) -> io::Result<Option<Playlist>> {
    let text = match fs::read_to_string(playlist_path(dir, id)) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let playlist =
        serde_json::from_str(&text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(Some(playlist))
}

pub fn save(dir: &Path, playlist: &Playlist) -> io::Result<()> {
    let path = playlist_path(dir, &playlist.id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Write to a temp file, then rename it over the real one.
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(playlist)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(&tmp, json)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

// Load the playlist, or create and save a fresh one on first run.
pub fn load_or_create(dir: &Path, id: &str, name: &str) -> io::Result<Playlist> {
    match load(dir, id)? {
        Some(playlist) => Ok(playlist),
        None => {
            let playlist = Playlist::new(id, name);
            save(dir, &playlist)?;
            Ok(playlist)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playlist::model::Track;

    #[test]
    fn save_then_load_round_trips() {
        let dir = std::env::temp_dir().join(format!("mpv-playlist-test-{}", uuid::Uuid::new_v4()));

        let mut playlist = Playlist::new("default", "Test");
        let track = Track::new("/videos/a.mp4");
        playlist.order.push(track.id.clone());
        playlist.tracks.push(track);

        save(&dir, &playlist).unwrap();
        let loaded = load(&dir, "default").unwrap().unwrap();

        assert_eq!(loaded.tracks.len(), 1);
        assert_eq!(loaded.order, playlist.order);

        let _ = fs::remove_dir_all(&dir);
    }
}
