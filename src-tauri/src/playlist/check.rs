use std::fs;
use std::io;

use super::model::Track;

// True only when we're SURE the file is gone, or is no longer a file.
// Errors that don't prove that (permissions, a flaky network drive) don't count.
fn is_missing(path: &str) -> bool {
    match fs::metadata(path) {
        Ok(meta) => !meta.is_file(), // a folder now sits where the video was
        Err(e) => e.kind() == io::ErrorKind::NotFound,
    }
}

// Returns the IDs of tracks whose files are missing.
pub fn find_missing(tracks: &[Track]) -> Vec<String> {
    tracks
        .iter()
        .filter(|track| is_missing(&track.path))
        .map(|track| track.id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_missing_files_and_replaced_folders_but_not_real_files() {
        let root = std::env::temp_dir().join(format!("mpv-check-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("a_folder")).unwrap();
        fs::write(root.join("real.mp4"), b"").unwrap();

        let real = Track::new(root.join("real.mp4").to_str().unwrap());
        let gone = Track::new(root.join("gone.mp4").to_str().unwrap());
        let folder = Track::new(root.join("a_folder").to_str().unwrap());

        let missing = find_missing(&[real, gone.clone(), folder.clone()]);
        assert_eq!(missing, vec![gone.id, folder.id]);

        let _ = fs::remove_dir_all(&root);
    }
}
