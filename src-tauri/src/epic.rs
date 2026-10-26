//! The Epic Games Launcher: is it installed, install it, open it on the Unreal Engine page.
//! Unreal Engine itself can only be installed from inside Epic's launcher (it needs an Epic sign-in),
//! so the app takes the creator to the right page and notices when 5.8 is there.

use crate::error::AppResult;
use crate::{applog, net, winutil};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use tauri_plugin_opener::OpenerExt;

/// Epic's own address for the launcher installer; it answers with a redirect to Epic's download servers.
const INSTALLER_URL: &str =
    "https://launcher-public-service-prod06.ol.epicgames.com/launcher/api/installer/download/EpicGamesLauncherInstaller.msi";
/// The installer must carry a valid signature from this organization, or it is not started.
const SIGNER: &str = "Epic Games Inc.";
/// Opens the launcher on its Unreal Engine library page.
const UNREAL_PAGE: &str = "com.epicgames.launcher://ue/library";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherStatus {
    pub installed: bool,
    pub exe: Option<String>,
}

#[cfg(windows)]
fn exe_from_protocol() -> Option<PathBuf> {
    use winreg::enums::HKEY_CLASSES_ROOT;
    use winreg::RegKey;
    let key = RegKey::predef(HKEY_CLASSES_ROOT).open_subkey(r"com.epicgames.launcher\shell\open\command").ok()?;
    let command: String = key.get_value("").ok()?;
    // The value looks like: "C:\...\EpicGamesLauncher.exe" %1
    let exe = command.trim().strip_prefix('"')?.split('"').next()?.to_string();
    Some(PathBuf::from(exe))
}

#[cfg(not(windows))]
fn exe_from_protocol() -> Option<PathBuf> {
    None
}

pub fn status() -> LauncherStatus {
    let mut candidates: Vec<PathBuf> = exe_from_protocol().into_iter().collect();
    for base in [r"C:\Program Files (x86)\Epic Games", r"C:\Program Files\Epic Games"] {
        for bits in ["Win64", "Win32"] {
            candidates.push(Path::new(base).join(format!(r"Launcher\Portal\Binaries\{bits}\EpicGamesLauncher.exe")));
        }
    }
    match candidates.into_iter().find(|p| p.is_file()) {
        Some(exe) => LauncherStatus { installed: true, exe: Some(exe.to_string_lossy().into_owned()) },
        None => LauncherStatus { installed: false, exe: None },
    }
}

/// Downloads Epic's installer, checks Epic's signature on it, then starts it. Epic's own installer
/// window takes over from there (Windows asks for permission, as for any install). When it closes,
/// the window gets event "epic-installer-closed" and looks again.
pub async fn install(app: AppHandle, download_dir: PathBuf) -> AppResult<()> {
    let msi = download_dir.join("EpicGamesLauncherInstaller.msi");
    applog::info("downloading the Epic Games Launcher installer");
    net::download(&app, "epic", "Downloading the Epic Games Launcher", INSTALLER_URL, &msi, None).await?;
    winutil::require_signed_by(&msi, SIGNER)?;
    applog::info("Epic's installer is signed by Epic Games Inc.; starting it");
    let mut child = std::process::Command::new("msiexec").arg("/i").arg(&msi).spawn()?;
    std::thread::spawn(move || {
        let code = child.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
        applog::info(&format!("Epic's installer closed (code {code})"));
        let _ = std::fs::remove_file(&msi);
        let _ = app.emit("epic-installer-closed", code);
    });
    Ok(())
}

/// Opens Epic's launcher on its Unreal Engine library, where "Install" for 5.8 lives.
pub fn open_unreal_page(app: &AppHandle) -> AppResult<()> {
    let st = status();
    if !st.installed {
        return crate::error::fail("Install the Epic Games Launcher first.");
    }
    if app.opener().open_url(UNREAL_PAGE, None::<&str>).is_err() {
        // The page address did not open: start the launcher itself.
        if let Some(exe) = st.exe {
            std::process::Command::new(exe).spawn()?;
        }
    }
    Ok(())
}
