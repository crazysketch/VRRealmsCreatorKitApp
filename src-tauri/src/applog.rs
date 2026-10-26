//! The app's own log: one text file in the app's data folder, plus the most recent lines in
//! memory for the Logs page. Nothing here leaves the PC.

use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const KEEP_LINES: usize = 400;
const ROTATE_BYTES: u64 = 2 * 1024 * 1024;

struct LogState {
    file: Option<File>,
    path: Option<PathBuf>,
    recent: VecDeque<String>,
}

static LOG: Mutex<LogState> = Mutex::new(LogState { file: None, path: None, recent: VecDeque::new() });

/// Opens `<data dir>/logs/app.log`. A log over 2 MB is kept as `app.old.log` and a new one started.
pub fn init(data_dir: &Path) {
    let dir = data_dir.join("logs");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("app.log");
    if fs::metadata(&path).map(|m| m.len() > ROTATE_BYTES).unwrap_or(false) {
        let _ = fs::rename(&path, dir.join("app.old.log"));
    }
    let file = OpenOptions::new().create(true).append(true).open(&path).ok();
    if let Ok(mut state) = LOG.lock() {
        state.file = file;
        state.path = Some(path);
    }
    info(&format!("VR Realms Creator Kit {} started", env!("CARGO_PKG_VERSION")));
}

fn write(level: &str, msg: &str) {
    let line = format!("{} [{}] {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), level, msg);
    if let Ok(mut state) = LOG.lock() {
        if let Some(f) = state.file.as_mut() {
            let _ = writeln!(f, "{line}");
        }
        state.recent.push_back(line);
        while state.recent.len() > KEEP_LINES {
            state.recent.pop_front();
        }
    }
}

pub fn info(msg: &str) {
    write("info", msg);
}

pub fn warn(msg: &str) {
    write("warn", msg);
}

pub fn error(msg: &str) {
    write("error", msg);
}

pub fn recent() -> Vec<String> {
    LOG.lock().map(|s| s.recent.iter().cloned().collect()).unwrap_or_default()
}

pub fn path() -> Option<PathBuf> {
    LOG.lock().ok().and_then(|s| s.path.clone())
}
