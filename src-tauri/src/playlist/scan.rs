use std::path::Path;

use walkdir::{DirEntry, WalkDir};

// The default list. A user-editable setting will replace this later.
pub const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "m2ts", "mts", "vob",
    "3gp", "ogv", "divx", "asf", "rmvb",
];

pub struct ScanResult {
    pub files: Vec<String>, // canonical, absolute paths
    pub unreadable: usize,  // things we had to skip because of an error
}

fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| VIDEO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

// Names starting with "." are hidden on Linux and macOS.
fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|name| name.starts_with('.'))
}

// Canonicalize a path and add it to the results, or count it as unreadable.
fn push_canonical(path: &Path, result: &mut ScanResult) {
    match dunce::canonicalize(path) {
        Ok(canonical) => match canonical.to_str() {
            Some(text) => result.files.push(text.to_string()),
            None => result.unreadable += 1, // not valid UTF-8, which we can't store
        },
        Err(_) => result.unreadable += 1, // missing, or no permission
    }
}

pub fn collect_videos(inputs: &[String]) -> ScanResult {
    let mut result = ScanResult {
        files: Vec::new(),
        unreadable: 0,
    };

    for input in inputs {
        let path = Path::new(input);

        if path.is_dir() {
            let walker = WalkDir::new(path)
                .sort_by_file_name()
                .into_iter()
                // depth 0 is the folder the user picked: never skip that one.
                .filter_entry(|entry| entry.depth() == 0 || !is_hidden(entry));

            for entry in walker {
                match entry {
                    Ok(entry) => {
                        // path().is_file() follows symlinks; file_type().is_file() doesn't.
                        if entry.path().is_file() && is_video(entry.path()) {
                            push_canonical(entry.path(), &mut result);
                        }
                    }
                    Err(_) => result.unreadable += 1,
                }
            }
        } else if path.is_file() {
            // The user picked this file on purpose, so skip the extension check.
            push_canonical(path, &mut result);
        } else {
            result.unreadable += 1; // doesn't exist, or isn't a file or folder
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_test_dir() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("mpv-scan-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::create_dir_all(root.join(".cache")).unwrap();
        for name in [
            "a.mp4",
            "B.MKV",
            "notes.txt",
            ".hidden.mp4",
            "sub/c.avi",
            ".cache/d.mp4",
        ] {
            fs::write(root.join(name), b"").unwrap();
        }
        root
    }

    fn file_names(files: &[String]) -> Vec<String> {
        let mut names: Vec<String> = files
            .iter()
            .map(|f| {
                Path::new(f)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }

    #[test]
    fn folder_scan_filters_extensions_and_skips_hidden() {
        let root = make_test_dir();
        let result = collect_videos(&[root.to_str().unwrap().to_string()]);

        // Included: a.mp4, B.MKV (case-insensitive), sub/c.avi (recursive).
        // Excluded: notes.txt (extension), .hidden.mp4 and .cache/ (hidden).
        assert_eq!(file_names(&result.files), vec!["B.MKV", "a.mp4", "c.avi"]);
        assert_eq!(result.unreadable, 0);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn explicitly_chosen_file_ignores_extension_filter() {
        let root = make_test_dir();
        let notes = root.join("notes.txt");
        let result = collect_videos(&[notes.to_str().unwrap().to_string()]);

        assert_eq!(file_names(&result.files), vec!["notes.txt"]);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_path_is_counted_as_unreadable() {
        let result = collect_videos(&["/definitely/not/a/real/path".to_string()]);
        assert!(result.files.is_empty());
        assert_eq!(result.unreadable, 1);
    }
}
