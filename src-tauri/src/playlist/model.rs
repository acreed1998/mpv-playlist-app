use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const FORMAT_VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: String,   // unique per entry, so the same file can appear twice
    pub path: String, // absolute path to the video
}

impl Track {
    pub fn new(path: impl Into<String>) -> Self {
        Track {
            id: uuid::Uuid::new_v4().to_string(),
            path: path.into(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SortKey {
    SourceOrder,
    Name,
    Modified,
    Directory,
    Length,
    Random,
    ShuffleFolders,
    Custom, // set after the user drags a track by hand
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    Ascending,
    Descending,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SortSettings {
    pub key: SortKey,
    pub direction: Direction,
    pub seed: Option<u64>, // the shuffle seed, if the sort is random-based
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ResumeState {
    pub track_id: String,
    pub position_secs: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub version: u32,
    pub id: String,
    pub name: String,
    pub allow_duplicates: bool,
    pub tracks: Vec<Track>, // in the order they were added
    pub order: Vec<String>, // track IDs in play order
    pub sort: SortSettings,
    pub resume: Option<ResumeState>,
}

impl Playlist {
    pub fn new(id: &str, name: &str) -> Self {
        Playlist {
            version: FORMAT_VERSION,
            id: id.to_string(),
            name: name.to_string(),
            allow_duplicates: false, // the default we settled on
            tracks: Vec::new(),
            order: Vec::new(),
            sort: SortSettings {
                key: SortKey::SourceOrder,
                direction: Direction::Ascending,
                seed: None,
            },
            resume: None,
        }
    }
}

// What happened during an add, so the UI can tell the user.
pub struct AddOutcome {
    pub added: usize,
    pub duplicates_skipped: usize,
}

impl Playlist {
    // Append canonical paths to the playlist, honoring the duplicate setting.
    pub fn add_files(&mut self, files: Vec<String>) -> AddOutcome {
        // When duplicates are disallowed, `seen` starts with every path already in the playlist.
        let mut seen: HashSet<String> = if self.allow_duplicates {
            HashSet::new()
        } else {
            self.tracks.iter().map(|track| track.path.clone()).collect()
        };

        let mut outcome = AddOutcome {
            added: 0,
            duplicates_skipped: 0,
        };

        for path in files {
            // HashSet::insert returns false if the value was already present.
            if !self.allow_duplicates && !seen.insert(path.clone()) {
                outcome.duplicates_skipped += 1;
                continue;
            }

            let track = Track::new(path);
            self.order.push(track.id.clone());
            self.tracks.push(track);
            outcome.added += 1;
        }

        outcome
    }

    // Move one track so it ends up at `to_index` in the play order.
    // Marks the playlist as hand-arranged if anything actually moved.
    pub fn move_track(&mut self, track_id: &str, to_index: usize) -> Result<(), String> {
        let from = self
            .order
            .iter()
            .position(|id| id == track_id)
            .ok_or("That track is not in the playlist")?;

        // Clamp so a too-large index means "the very end".
        let to = to_index.min(self.order.len() - 1);
        if from == to {
            return Ok(()); // dropped where it started: change nothing
        }

        let id = self.order.remove(from);
        self.order.insert(to, id);
        self.sort = SortSettings {
            key: SortKey::Custom,
            direction: Direction::Ascending,
            seed: None,
        };
        Ok(())
    }

    // Remove tracks by ID. Returns how many were actually removed.
    pub fn remove_tracks(&mut self, ids: &[String]) -> usize {
        // A set gives instant "is this ID being removed?" checks. It borrows from `ids`.
        let doomed: HashSet<&str> = ids.iter().map(|id| id.as_str()).collect();
        let before = self.tracks.len();

        self.tracks
            .retain(|track| !doomed.contains(track.id.as_str()));
        self.order.retain(|id| !doomed.contains(id.as_str()));

        // A resume point into a removed track would point at nothing.
        if self
            .resume
            .as_ref()
            .is_some_and(|saved| doomed.contains(saved.track_id.as_str()))
        {
            self.resume = None;
        }

        before - self.tracks.len()
    }

    // Empty the playlist but keep its identity and the duplicate preference.
    pub fn clear(&mut self) {
        self.tracks.clear();
        self.order.clear();
        self.resume = None;
        self.sort = SortSettings {
            key: SortKey::SourceOrder,
            direction: Direction::Ascending,
            seed: None,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(list: &[&str]) -> Vec<String> {
        list.iter().map(|p| p.to_string()).collect()
    }

    #[test]
    fn duplicates_are_skipped_by_default() {
        let mut playlist = Playlist::new("t", "Test");
        playlist.add_files(paths(&["/v/a.mp4", "/v/b.mp4"]));

        // "a.mp4" is already present, and appears twice more in this batch.
        let outcome = playlist.add_files(paths(&["/v/a.mp4", "/v/c.mp4", "/v/c.mp4"]));

        assert_eq!(outcome.added, 1); // only c.mp4, once
        assert_eq!(outcome.duplicates_skipped, 2);
        assert_eq!(playlist.tracks.len(), 3);
        assert_eq!(playlist.order.len(), 3);
    }

    fn ids_to_names(p: &Playlist) -> Vec<String> {
        p.order
            .iter()
            .map(|id| {
                let track = p.tracks.iter().find(|t| &t.id == id).unwrap();
                track.path.trim_start_matches("/v/").to_string()
            })
            .collect()
    }

    fn four_tracks() -> Playlist {
        let mut p = Playlist::new("t", "Test");
        p.add_files(paths(&["/v/a", "/v/b", "/v/c", "/v/d"]));
        p
    }

    #[test]
    fn duplicates_are_kept_when_allowed() {
        let mut playlist = Playlist::new("t", "Test");
        playlist.allow_duplicates = true;

        let outcome = playlist.add_files(paths(&["/v/a.mp4", "/v/a.mp4"]));

        assert_eq!(outcome.added, 2);
        assert_eq!(outcome.duplicates_skipped, 0);
        // Same file twice, but each entry has its own ID.
        assert_ne!(playlist.tracks[0].id, playlist.tracks[1].id);
    }

    #[test]
    fn moving_a_track_forward_and_backward() {
        let mut p = four_tracks();
        let a = p.order[0].clone();
        let d = p.order[3].clone();

        p.move_track(&a, 2).unwrap();
        assert_eq!(ids_to_names(&p), vec!["b", "c", "a", "d"]);

        p.move_track(&d, 0).unwrap();
        assert_eq!(ids_to_names(&p), vec!["d", "b", "c", "a"]);
        assert_eq!(p.sort.key, SortKey::Custom);
    }

    #[test]
    fn moving_to_the_same_spot_keeps_the_sort_label() {
        let mut p = four_tracks();
        let b = p.order[1].clone();

        p.move_track(&b, 1).unwrap();
        assert_eq!(p.sort.key, SortKey::SourceOrder);
    }

    #[test]
    fn oversized_index_means_the_end_and_unknown_id_is_an_error() {
        let mut p = four_tracks();
        let a = p.order[0].clone();

        p.move_track(&a, 999).unwrap();
        assert_eq!(ids_to_names(&p), vec!["b", "c", "d", "a"]);
        assert!(p.move_track("no-such-id", 0).is_err());
    }

    #[test]
    fn removing_tracks_keeps_the_rest_in_order_and_the_sort_label() {
        let mut p = four_tracks();
        let b = p.order[1].clone();
        let d = p.order[3].clone();

        let removed = p.remove_tracks(&[b, d, "no-such-id".to_string()]);

        assert_eq!(removed, 2); // the unknown ID is ignored, not counted
        assert_eq!(ids_to_names(&p), vec!["a", "c"]);
        assert_eq!(p.tracks.len(), 2); // `tracks` and `order` stay in step
        assert_eq!(p.sort.key, SortKey::SourceOrder);
    }

    #[test]
    fn removing_the_resume_track_clears_the_resume_point() {
        let mut p = four_tracks();
        let a = p.order[0].clone();
        let b = p.order[1].clone();
        p.resume = Some(ResumeState {
            track_id: a.clone(),
            position_secs: 5.0,
        });

        p.remove_tracks(&[b]);
        assert!(p.resume.is_some()); // a different track: resume untouched

        p.remove_tracks(&[a]);
        assert!(p.resume.is_none());
    }

    #[test]
    fn clearing_resets_the_playlist_but_keeps_the_duplicate_setting() {
        let mut p = four_tracks();
        p.allow_duplicates = true;
        let a = p.order[0].clone();
        p.move_track(&a, 2).unwrap(); // makes the sort "custom"
        p.resume = Some(ResumeState {
            track_id: p.order[0].clone(),
            position_secs: 5.0,
        });

        p.clear();

        assert!(p.tracks.is_empty() && p.order.is_empty());
        assert!(p.resume.is_none());
        assert_eq!(p.sort.key, SortKey::SourceOrder);
        assert!(p.allow_duplicates);
    }
}
