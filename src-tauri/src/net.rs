//! Downloads, with progress for the window and an optional SHA-256 check.

use crate::error::{fail, AppError, AppResult};
use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Runtime};

pub const USER_AGENT: &str = concat!("VRRealmsCreatorKit/", env!("CARGO_PKG_VERSION"));

/// No download of this app is near this size; a bigger one is stopped before it fills the disk.
const MAX_DOWNLOAD: u64 = 6 * 1024 * 1024 * 1024;

/// Sent to the window as event "progress" while something downloads or installs.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub task: String,
    pub label: String,
    pub done: u64,
    pub total: Option<u64>,
}

pub fn emit_progress<R: Runtime>(app: &AppHandle<R>, task: &str, label: &str, done: u64, total: Option<u64>) {
    let _ = app.emit(
        "progress",
        ProgressEvent { task: task.into(), label: label.into(), done, total },
    );
}

pub fn client(timeout: Duration) -> AppResult<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(20))
        .timeout(timeout)
        .build()?)
}

pub async fn get_json<T: serde::de::DeserializeOwned>(url: &str, timeout_secs: u64) -> AppResult<T> {
    let resp = client(Duration::from_secs(timeout_secs))?.get(url).send().await?;
    let status = resp.status();
    if status.as_u16() == 403 || status.as_u16() == 429 {
        return fail("GitHub is limiting requests from this connection right now. Try again in a few minutes.");
    }
    if !status.is_success() {
        return Err(AppError::Network(format!("{url} answered {status}")));
    }
    Ok(resp.json::<T>().await?)
}

/// Downloads `url` into `dest`. With `expect_sha256` the file is kept only when its hash matches
/// (compared without regard to upper / lower case); otherwise it is deleted and nothing else happens.
pub async fn download<R: Runtime>(
    app: &AppHandle<R>,
    task: &str,
    label: &str,
    url: &str,
    dest: &Path,
    expect_sha256: Option<&str>,
) -> AppResult<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let part = dest.with_extension("part");
    let resp = client(Duration::from_secs(30 * 60))?.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(AppError::Network(format!("the download answered {}", resp.status())));
    }
    let total = resp.content_length();
    if total.unwrap_or(0) > MAX_DOWNLOAD {
        return fail("That download is far larger than it should be, so it was not started.");
    }
    let mut stream = resp.bytes_stream();
    let mut file = File::create(&part)?;
    let mut hasher = Sha256::new();
    let mut done: u64 = 0;
    let mut last = Instant::now() - Duration::from_secs(1);
    emit_progress(app, task, label, 0, total);
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AppError::Network(e.to_string()))?;
        file.write_all(&chunk)?;
        hasher.update(&chunk);
        done += chunk.len() as u64;
        if done > MAX_DOWNLOAD {
            drop(file);
            let _ = fs::remove_file(&part);
            return fail("That download grew far larger than it should be, so it was stopped and deleted.");
        }
        if last.elapsed() >= Duration::from_millis(200) {
            emit_progress(app, task, label, done, total);
            last = Instant::now();
        }
    }
    file.flush()?;
    drop(file);
    emit_progress(app, task, label, done, total);

    if let Some(want) = expect_sha256 {
        let got = hex::encode(hasher.finalize());
        if !got.eq_ignore_ascii_case(want.trim()) {
            let _ = fs::remove_file(&part);
            return fail(format!(
                "The download did not match its published checksum, so it was not used. Nothing was changed. \
                 (expected {want}, got {got})"
            ));
        }
    }
    if dest.exists() {
        fs::remove_file(dest)?;
    }
    fs::rename(&part, dest)?;
    Ok(())
}

/// SHA-256 of a file on disk, lower-case hex.
pub fn sha256_file(path: &Path) -> AppResult<String> {
    let mut f = File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut f, &mut hasher)?;
    Ok(hex::encode(hasher.finalize()))
}
