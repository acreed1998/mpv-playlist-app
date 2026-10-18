use std::io;
use std::time::Duration;

#[cfg(unix)]
pub const IPC_PATH: &str = "/tmp/mpv-playlist-app.sock";
#[cfg(windows)]
pub const IPC_PATH: &str = r"\\.\pipe\mpv-playlist-app";

// The connection type differs per platform, but both can Read and Write.
#[cfg(unix)]
pub type Conn = std::os::unix::net::UnixStream;
#[cfg(windows)]
pub type Conn = std::fs::File;

#[cfg(unix)]
fn connect() -> io::Result<Conn> {
    Conn::connect(IPC_PATH)
}

#[cfg(windows)]
fn connect() -> io::Result<Conn> {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(IPC_PATH)
}

// mpv needs a moment after launching before its socket exists. Try for ~5 seconds.
pub fn connect_with_retry() -> io::Result<Conn> {
    let mut last_err = None;
    for _ in 0..50 {
        match connect() {
            Ok(conn) => return Ok(conn),
            Err(e) => {
                last_err = Some(e);
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
    Err(last_err.unwrap())
}

// A killed mpv can't clean up after itself, so its socket file stays behind.
// Deleting it before launching means we can only ever connect to the NEW mpv.
// Windows named pipes vanish with their process, so there's nothing to do there.
pub fn remove_stale_socket() {
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(IPC_PATH);
    }
}
