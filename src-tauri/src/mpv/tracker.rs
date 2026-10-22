use serde_json::Value;

// What the tracker wants the outside world to do.
#[derive(Debug, PartialEq)]
pub enum Update {
    // Record "we're on this track, this many seconds in".
    // `urgent` means a track change: save to disk now, don't wait for the timer.
    Progress {
        track_id: String,
        secs: f64,
        urgent: bool,
    },
    // Tell mpv to jump to this position (resuming mid-track).
    Seek(f64),
    // The last track ended naturally: the playlist is done.
    Finished,
    // This track ended naturally and MPV should be relaunched on the next one.
    RestartAt(usize),
}

pub struct Tracker {
    track_ids: Vec<String>, // the queue as launched, so index N means track_ids[N]
    start_index: usize,     // where mpv was told to begin
    pending_seek: Option<f64>, // a resume position we still need to apply
    index: Option<usize>,   // the entry mpv says is playing right now
    finished: bool,
    restart_mpv_on_track_end: bool,
}

impl Tracker {
    pub fn new(
        track_ids: Vec<String>,
        start_index: usize,
        start_secs: f64,
        restart_mpv_on_track_end: bool,
    ) -> Self {
        Tracker {
            track_ids,
            start_index,
            pending_seek: (start_secs > 0.0).then_some(start_secs),
            index: restart_mpv_on_track_end.then_some(start_index),
            finished: false,
            restart_mpv_on_track_end,
        }
    }

    // Look at one JSON message from mpv. Most messages need no action.
    pub fn handle(&mut self, msg: &Value) -> Option<Update> {
        if self.finished {
            return None; // nothing after the end may undo "finished"
        }
        // Replies to our commands have no "event" key, so `?` skips them here.
        match msg["event"].as_str()? {
            "property-change" => match msg["name"].as_str()? {
                "playlist-pos" => self.on_position(msg["data"].as_i64()?),
                "time-pos" => self.on_time(msg["data"].as_f64()?),
                _ => None,
            },
            "end-file" => self.on_end(msg["reason"].as_str()),
            _ => None,
        }
    }

    fn on_position(&mut self, raw: i64) -> Option<Update> {
        // mpv reports -1 when nothing is playing; that doesn't fit a usize, so it's skipped.
        let reported_index = usize::try_from(raw).ok()?;
        let index = if self.restart_mpv_on_track_end {
            // In restart mode each MPV process receives a one-entry playlist.
            if reported_index != 0 {
                return None;
            }
            self.start_index
        } else {
            reported_index
        };
        let track_id = self.track_ids.get(index)?.clone();

        self.index = Some(index);
        // If playback began somewhere else (say the first file was unplayable),
        // the saved position no longer applies.
        if index != self.start_index {
            self.pending_seek = None;
        }

        Some(Update::Progress {
            track_id,
            secs: self.pending_seek.unwrap_or(0.0),
            urgent: true,
        })
    }

    fn on_time(&mut self, secs: f64) -> Option<Update> {
        let index = self.index?;

        // The first time mpv reports a playback time, the file is loaded, so
        // this is the moment to jump to the saved position. We don't record
        // this "0.0" as progress, or a quick quit would lose the saved spot.
        if let Some(target) = self.pending_seek.take() {
            return Some(Update::Seek(target));
        }

        let track_id = self.track_ids.get(index)?.clone();
        Some(Update::Progress {
            track_id,
            secs,
            urgent: false,
        })
    }

    fn on_end(&mut self, reason: Option<&str>) -> Option<Update> {
        let ended_by_itself = matches!(reason, Some("eof") | Some("error"));
        let on_last_track =
            self.index.is_some() && self.index == self.track_ids.len().checked_sub(1);

        if !ended_by_itself {
            return None;
        }

        if on_last_track {
            self.finished = true;
            return Some(Update::Finished);
        }

        if self.restart_mpv_on_track_end {
            if let Some(index) = self.index {
                self.finished = true;
                return Some(Update::RestartAt(index + 1));
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ids(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("t{i}")).collect()
    }
    fn pos(i: i64) -> Value {
        json!({"event": "property-change", "id": 1, "name": "playlist-pos", "data": i})
    }
    fn time(secs: f64) -> Value {
        json!({"event": "property-change", "id": 2, "name": "time-pos", "data": secs})
    }
    fn end(reason: &str) -> Value {
        json!({"event": "end-file", "reason": reason})
    }
    fn progress(id: &str, secs: f64, urgent: bool) -> Option<Update> {
        Some(Update::Progress {
            track_id: id.to_string(),
            secs,
            urgent,
        })
    }

    #[test]
    fn progress_follows_the_track_mpv_reports() {
        let mut t = Tracker::new(ids(3), 0, 0.0, false);
        assert_eq!(t.handle(&pos(0)), progress("t0", 0.0, true));
        assert_eq!(t.handle(&time(12.5)), progress("t0", 12.5, false));
        assert_eq!(t.handle(&pos(1)), progress("t1", 0.0, true));
    }

    #[test]
    fn resume_seeks_once_then_records_normally() {
        let mut t = Tracker::new(ids(3), 1, 90.0, false);
        // The saved spot is what gets recorded while we wait for the file to load.
        assert_eq!(t.handle(&pos(1)), progress("t1", 90.0, true));
        assert_eq!(t.handle(&time(0.0)), Some(Update::Seek(90.0)));
        assert_eq!(t.handle(&time(91.0)), progress("t1", 91.0, false));
    }

    #[test]
    fn saved_position_is_dropped_if_playback_starts_elsewhere() {
        let mut t = Tracker::new(ids(3), 1, 90.0, false);
        assert_eq!(t.handle(&pos(2)), progress("t2", 0.0, true)); // mpv skipped track 1
        assert_eq!(t.handle(&time(1.0)), progress("t2", 1.0, false)); // no seek
    }

    #[test]
    fn nulls_replies_and_unrelated_events_are_ignored() {
        let mut t = Tracker::new(ids(2), 0, 0.0, false);
        let null_time = json!({"event": "property-change", "name": "time-pos", "data": null});
        assert_eq!(t.handle(&null_time), None);
        assert_eq!(t.handle(&pos(-1)), None);
        assert_eq!(t.handle(&json!({"event": "pause"})), None);
        assert_eq!(t.handle(&json!({"error": "success", "data": null})), None);
    }

    #[test]
    fn only_a_natural_end_of_the_last_track_finishes_the_playlist() {
        let mut t = Tracker::new(ids(2), 0, 0.0, false);
        t.handle(&pos(0));
        assert_eq!(t.handle(&end("eof")), None); // first of two: not done
        t.handle(&pos(1));
        assert_eq!(t.handle(&end("quit")), None); // user closed mpv: keep the position
        assert_eq!(t.handle(&end("eof")), Some(Update::Finished));
        assert_eq!(t.handle(&time(5.0)), None); // nothing after the end counts
    }

    #[test]
    fn restart_mode_advances_to_the_next_track_then_finishes() {
        let mut t = Tracker::new(ids(3), 1, 0.0, true);
        assert_eq!(t.handle(&pos(0)), progress("t1", 0.0, true));
        assert_eq!(t.handle(&end("eof")), Some(Update::RestartAt(2)));
        assert_eq!(t.handle(&end("eof")), None);

        let mut last = Tracker::new(ids(3), 2, 0.0, true);
        assert_eq!(last.handle(&end("eof")), Some(Update::Finished));
        assert_eq!(last.handle(&time(10.0)), None);
    }

    #[test]
    fn restart_mode_does_not_advance_after_manual_quit() {
        let mut t = Tracker::new(ids(3), 1, 0.0, true);
        assert_eq!(t.handle(&end("quit")), None);
    }
}
