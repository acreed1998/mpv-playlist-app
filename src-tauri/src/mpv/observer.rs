use std::io::{self, BufRead, BufReader, Write};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager};

use super::ipc;
use super::process;
use super::tracker::{Tracker, Update};
use crate::playlist::{model::ResumeState, storage};
use crate::state::{MpvState, PlaylistState};

const PROGRESS_EVERY: Duration = Duration::from_secs(1); // memory + UI updates
const SAVE_EVERY: Duration = Duration::from_secs(5); // disk writes

const CONNECT_FAILED: &str = "Couldn't connect to mpv's control socket, so your position isn't \
being saved and Resume won't work for this session. If mpv was installed as a Snap or Flatpak, \
its sandbox can hide the socket: install the regular package instead (for example \
`sudo apt install mpv`), or choose a different mpv in Settings.";

// Everything the observer needs to know about the launch it's watching.
pub struct Session {
    pub track_ids: Vec<String>,
    pub paths: Vec<String>,
    pub start_index: usize,
    pub start_secs: f64,
    pub restart_mpv_on_track_end: bool,
    pub program: String,
    pub playback_speed: String,
}

// "Has enough time passed since the last time this said yes?"
struct Throttle {
    every: Duration,
    last: Option<Instant>,
}

impl Throttle {
    fn new(every: Duration) -> Self {
        Throttle { every, last: None }
    }

    fn ready(&mut self) -> bool {
        let due = self.last.map_or(true, |t| t.elapsed() >= self.every);
        if due {
            self.last = Some(Instant::now());
        }
        due
    }
}

// Start watching on a new thread and return immediately.
pub fn spawn(app: AppHandle, generation: u64, session: Session) {
    std::thread::spawn(move || {
        if let Err(e) = observe(&app, generation, session) {
            eprintln!("mpv observer stopped: {e}");
        }
    });
}

fn send(conn: &mut ipc::Conn, command: Value) -> io::Result<()> {
    writeln!(conn, "{}", json!({ "command": command }))
}

fn observe(app: &AppHandle, generation: u64, mut session: Session) -> Result<(), String> {
    'sessions: loop {
        let conn = match ipc::connect_with_retry() {
            Ok(conn) => conn,
            Err(e) => {
                eprintln!("mpv observer could not connect: {e}");
                report_tracking_failure(app, generation, CONNECT_FAILED);
                return Ok(());
            }
        };

        let mut reader = BufReader::new(conn);

        // Ask mpv to push us an event whenever these change.
        for (id, name) in [(1, "playlist-pos"), (2, "time-pos")] {
            send(reader.get_mut(), json!(["observe_property", id, name]))
                .map_err(|e| format!("could not talk to mpv: {e}"))?;
        }

        let mut tracker = Tracker::new(
            session.track_ids.clone(),
            session.start_index,
            session.start_secs,
            session.restart_mpv_on_track_end,
        );
        let mut progress_timer = Throttle::new(PROGRESS_EVERY);
        let mut save_timer = Throttle::new(SAVE_EVERY);
        let mut line = String::new();

        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break, // mpv closed the connection (it exited, or was killed)
                Ok(_) => {}
            }

            let Ok(msg) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            let Some(update) = tracker.handle(&msg) else {
                continue;
            };

            match update {
                Update::Seek(secs) => {
                    send(reader.get_mut(), json!(["seek", secs, "absolute"]))
                        .map_err(|e| format!("could not talk to mpv: {e}"))?;
                }
                Update::Progress {
                    track_id,
                    secs,
                    urgent,
                } => {
                    let due = progress_timer.ready();
                    if !urgent && !due {
                        continue;
                    }
                    let save_now = save_timer.ready() || urgent;
                    let resume = Some(ResumeState {
                        track_id,
                        position_secs: secs,
                    });

                    if !record(app, generation, resume.clone(), save_now) {
                        return Ok(()); // a newer session took over
                    }
                    let _ = app.emit("resume-changed", resume);
                }
                Update::Finished => {
                    if !record(app, generation, None, true) {
                        return Ok(());
                    }
                    let _ = app.emit("resume-changed", None::<ResumeState>);
                }
                Update::RestartAt(index) => {
                    let Some(path) = session.paths.get(index) else {
                        return Err(format!("No video exists at playlist position {index}."));
                    };
                    let Some(track_id) = session.track_ids.get(index) else {
                        return Err(format!("No track ID exists at playlist position {index}."));
                    };
                    let mpv_state = app.state::<MpvState>();
                    let restarted = match process::restart_mpv_for_track(
                        &mpv_state,
                        generation,
                        &session.program,
                        path,
                        &session.playback_speed,
                    ) {
                        Ok(restarted) => restarted,
                        Err(error) => {
                            let message =
                                format!("Could not reopen MPV for the next video: {error}");
                            report_tracking_failure(app, generation, &message);
                            return Err(message);
                        }
                    };
                    if !restarted {
                        return Ok(());
                    }

                    let resume = Some(ResumeState {
                        track_id: track_id.clone(),
                        position_secs: 0.0,
                    });
                    if !record(app, generation, resume.clone(), true) {
                        return Ok(());
                    }
                    let _ = app.emit("resume-changed", resume);
                    session.start_index = index;
                    session.start_secs = 0.0;
                    continue 'sessions;
                }
            }
        }

        flush(app, generation); // mpv is gone: make sure the last position is on disk
        return Ok(());
    }
}

// Update the in-memory playlist (and optionally the file).
// Returns false if this observer is out of date and should stop.
fn record(app: &AppHandle, generation: u64, resume: Option<ResumeState>, save_now: bool) -> bool {
    let playlist_state = app.state::<PlaylistState>();
    let mpv_state = app.state::<MpvState>();

    let Ok(mut playlist) = playlist_state.playlist.lock() else {
        return false;
    };
    // Checked while holding the lock, so shutdown can't slip in between check and write.
    if mpv_state.generation.load(Ordering::SeqCst) != generation {
        return false;
    }

    playlist.resume = resume;
    if save_now {
        let _ = storage::save(&playlist_state.dir, &playlist);
    }
    true
}

fn flush(app: &AppHandle, generation: u64) {
    let playlist_state = app.state::<PlaylistState>();
    let mpv_state = app.state::<MpvState>();

    let Ok(playlist) = playlist_state.playlist.lock() else {
        return;
    };
    if mpv_state.generation.load(Ordering::SeqCst) != generation {
        return;
    }
    let _ = storage::save(&playlist_state.dir, &playlist);
}

// Tell the UI that tracking isn't working, unless this observer is already out of date.
fn report_tracking_failure(app: &AppHandle, generation: u64, message: &str) {
    let mpv_state = app.state::<MpvState>();
    if mpv_state.generation.load(Ordering::SeqCst) != generation {
        return; // the user stopped or restarted mpv; this failure no longer matters
    }
    let _ = app.emit("tracking-status", Some(message));
}
