use std::collections::HashMap;

use super::model::Playlist;

pub struct Queue {
    pub paths: Vec<String>, // every track, in play order
    pub track_ids: Vec<String>, // the same tracks' IDs, in the same order
    pub start_index: usize, // which entry mpv should begin on
}

// Turn the playlist's ID order into a list of paths, and find the start position.
// `start_track_id` of None means "from the top".
pub fn build_queue(playlist: &Playlist, start_track_id: Option<&str>) -> Result<Queue, String> {
    // A lookup table, so each ID in `order` is found instantly.
    let path_by_id: HashMap<&str, &str> = playlist
        .tracks
        .iter()
        .map(|track| (track.id.as_str(), track.path.as_str()))
        .collect();

    // (id, path) pairs in play order. An ID with no matching track is skipped,
    // which can only happen if the saved file was edited by hand.
    let entries: Vec<(&str, &str)> = playlist
        .order
        .iter()
        .filter_map(|id| path_by_id.get(id.as_str()).map(|path| (id.as_str(), *path)))
        .collect();

    if entries.is_empty() {
        return Err("The playlist is empty.".to_string());
    }

    let start_index = match start_track_id {
        None => 0,
        Some(wanted) => entries
            .iter()
            .position(|(id, _)| *id == wanted)
            .ok_or("That track is not in the playlist.")?,
    };

    let track_ids: Vec<String> = entries.iter().map(|(id, _)| id.to_string()).collect();

    Ok(Queue {
        paths: entries.into_iter().map(|(_, path)| path.to_string()).collect(),
        track_ids,
        start_index,
    })
}

// The contents of the file we give to mpv's --playlist option.
pub fn queue_file_text(paths: &[String]) -> String {
    let mut text = String::from("#EXTM3U\n"); // header that identifies the m3u format
    for path in paths {
        text.push_str(path);
        text.push('\n');
    }
    text
}

pub struct StartPoint {
    pub track_id: Option<String>, // None means "from the top"
    pub seconds: f64,
}

// Decide where to begin:
//  1. A track the user explicitly chose ("play from here") starts at 0:00.
//  2. Otherwise, if resuming and the saved track is still in the playlist, continue there.
//  3. Otherwise start from the top.
pub fn resolve_start(playlist: &Playlist, requested: Option<&str>, use_resume: bool) -> StartPoint {
    if let Some(id) = requested {
        return StartPoint { track_id: Some(id.to_string()), seconds: 0.0 };
    }
    if use_resume {
        if let Some(saved) = &playlist.resume {
            if playlist.order.iter().any(|id| *id == saved.track_id) {
                return StartPoint {
                    track_id: Some(saved.track_id.clone()),
                    seconds: saved.position_secs,
                };
            }
        }
    }
    StartPoint { track_id: None, seconds: 0.0 }
}


#[cfg(test)]
mod tests {
    use crate::playlist::model::ResumeState;
    use super::*;

    fn playlist_abc() -> Playlist {
        let mut p = Playlist::new("t", "Test");
        p.add_files(vec![
            "/v/a.mp4".into(),
            "/v/b.mp4".into(),
            "/v/c.mp4".into(),
        ]);
        p
    }

    #[test]
    fn queue_follows_play_order_not_added_order() {
        let mut p = playlist_abc();
        let c = p.order[2].clone();
        p.move_track(&c, 0).unwrap(); // c now plays first

        let queue = build_queue(&p, None).unwrap();
        assert_eq!(queue.paths, vec!["/v/c.mp4", "/v/a.mp4", "/v/b.mp4"]);
        assert_eq!(queue.start_index, 0);
    }

    #[test]
    fn start_index_points_at_the_chosen_track_and_list_stays_whole() {
        let p = playlist_abc();
        let b = p.order[1].clone();

        let queue = build_queue(&p, Some(b.as_str())).unwrap();
        assert_eq!(queue.start_index, 1);
        assert_eq!(queue.paths.len(), 3); // never truncated
    }

    #[test]
    fn unknown_start_track_is_an_error() {
        let p = playlist_abc();
        assert!(build_queue(&p, Some("no-such-id")).is_err());
    }

    #[test]
    fn empty_playlist_is_an_error() {
        let p = Playlist::new("t", "Test");
        assert!(build_queue(&p, None).is_err());
    }

    #[test]
    fn queue_file_has_a_header_and_one_path_per_line() {
        let text = queue_file_text(&["/v/a.mp4".to_string(), "/v/b.mp4".to_string()]);
        assert_eq!(text, "#EXTM3U\n/v/a.mp4\n/v/b.mp4\n");
    }

    #[test]
    fn explicit_track_wins_over_saved_position() {
        let mut p = playlist_abc();
        p.resume = Some(ResumeState { track_id: p.order[2].clone(), position_secs: 50.0 });
        let b = p.order[1].clone();

        let start = resolve_start(&p, Some(b.as_str()), true);
        assert_eq!(start.track_id, Some(b));
        assert_eq!(start.seconds, 0.0);
    }

    #[test]
    fn resume_uses_the_saved_track_and_position_only_when_asked() {
        let mut p = playlist_abc();
        let c = p.order[2].clone();
        p.resume = Some(ResumeState { track_id: c.clone(), position_secs: 50.0 });

        let resumed = resolve_start(&p, None, true);
        assert_eq!(resumed.track_id, Some(c));
        assert_eq!(resumed.seconds, 50.0);

        let fresh = resolve_start(&p, None, false);
        assert_eq!(fresh.track_id, None);
        assert_eq!(fresh.seconds, 0.0);
    }

    #[test]
    fn resume_pointing_at_a_missing_track_falls_back_to_the_top() {
        let mut p = playlist_abc();
        p.resume = Some(ResumeState { track_id: "gone".into(), position_secs: 50.0 });

        let start = resolve_start(&p, None, true);
        assert_eq!(start.track_id, None);
    }    
}
