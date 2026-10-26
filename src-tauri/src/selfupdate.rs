//! The app updating itself. A copy installed with VRRealmsCreatorKitApp-Setup.exe asks the newest GitHub release for its
//! latest.json, downloads the installer it names, and runs it only when the installer's signature checks out against the
//! public key built into the app (tauri.conf.json). The installer closes the app, puts the new version in place and
//! starts it again. A Steam copy is left to Steam; a copy run without installing (the plain .exe) is pointed at the
//! download instead, since an installer would put a second copy beside it.

use crate::error::{fail, AppError, AppResult};
use crate::{applog, net};
use serde::Serialize;
use std::path::Path;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

pub const DOWNLOAD_PAGE: &str = "https://github.com/crazysketch/VRRealmsCreatorKitApp/releases/latest";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// In Steam's library: Steam keeps it up to date.
    Steam,
    /// Installed by our installer (its uninstaller sits beside it): it can update itself.
    Installed,
    /// The plain .exe, run from wherever it was saved.
    Portable,
}

impl Kind {
    fn word(self) -> &'static str {
        match self {
            Kind::Steam => "steam",
            Kind::Installed => "installed",
            Kind::Portable => "portable",
        }
    }
}

/// How this copy of the app was put on the PC, from where its exe lies.
pub fn kind_of(exe: &Path) -> Kind {
    let text = exe.to_string_lossy().replace('/', "\\").to_ascii_lowercase();
    if text.contains("\\steamapps\\common\\") {
        return Kind::Steam;
    }
    if exe.parent().map(|d| d.join("uninstall.exe").is_file()).unwrap_or(false) {
        return Kind::Installed;
    }
    Kind::Portable
}

fn this_kind() -> Kind {
    std::env::current_exe().map(|e| kind_of(&e)).unwrap_or(Kind::Portable)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdate {
    pub current: String,
    pub latest: Option<String>,
    pub has_update: bool,
    pub notes: String,
    /// "installed" (updates itself), "steam" (Steam does it) or "portable" (download the new one).
    pub how: String,
    pub download_page: String,
}

fn updater_error(e: tauri_plugin_updater::Error) -> AppError {
    AppError::Network(format!("the app's update check failed: {e}"))
}

pub async fn check(app: &AppHandle) -> AppResult<AppUpdate> {
    let current = app.package_info().version.to_string();
    let kind = this_kind();
    let mut out = AppUpdate {
        current,
        latest: None,
        has_update: false,
        notes: String::new(),
        how: kind.word().into(),
        download_page: DOWNLOAD_PAGE.into(),
    };
    // Steam replaces a Steam copy by itself; telling its owner about a GitHub release Steam may not carry yet would mislead.
    if kind == Kind::Steam {
        return Ok(out);
    }
    let update = app.updater_builder().build().map_err(updater_error)?.check().await.map_err(updater_error)?;
    if let Some(u) = update {
        out.latest = Some(u.version.clone());
        out.has_update = true;
        out.notes = u.body.clone().unwrap_or_default().chars().take(3000).collect();
    }
    Ok(out)
}

/// Downloads the new version, checks its signature and runs its installer, which closes this app and starts the new one.
/// Returns only when something went wrong.
pub async fn install(app: AppHandle) -> AppResult<()> {
    if this_kind() != Kind::Installed {
        return fail("This copy of the app cannot update itself. Download the new version from the release page.");
    }
    let Some(update) = app.updater_builder().build().map_err(updater_error)?.check().await.map_err(updater_error)? else {
        return fail("There is no newer version of the app.");
    };
    let to = update.version.clone();
    applog::info(&format!("app update: downloading {to}"));
    let label = format!("Downloading the app, version {to}");
    let mut done: u64 = 0;
    let progress_app = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                done += chunk as u64;
                net::emit_progress(&progress_app, "app-update", &label, done, total);
            },
            || applog::info("app update: downloaded, signature checked, starting the installer"),
        )
        .await
        .map_err(|e| AppError::Message(format!("The app's update was not installed: {e}. Nothing was changed.")))?;
    // On Windows the installer has taken over and this process has already ended; nothing below runs there.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn tells_how_a_copy_was_put_on_the_pc() {
        assert_eq!(kind_of(Path::new(r"C:\Program Files (x86)\Steam\steamapps\common\VR Realms Creator Kit\VRRealmsCreatorKit.exe")), Kind::Steam);
        assert_eq!(kind_of(Path::new(r"D:\SteamLibrary\SteamApps\Common\Kit\VRRealmsCreatorKit.exe")), Kind::Steam);
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("VRRealmsCreatorKit.exe");
        fs::write(&exe, b"x").unwrap();
        assert_eq!(kind_of(&exe), Kind::Portable);
        fs::write(dir.path().join("uninstall.exe"), b"x").unwrap();
        assert_eq!(kind_of(&exe), Kind::Installed);
    }
}
