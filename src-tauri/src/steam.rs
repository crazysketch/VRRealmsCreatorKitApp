//! Steam and SteamCMD. Workshop uploads from the kit go through SteamCMD, Valve's own uploader,
//! signed in once with the creator's Steam account. This app never sees or stores a password:
//! the sign-in happens in SteamCMD's own window.

use crate::error::{fail, AppResult};
use crate::{applog, net, winutil};
use serde::Serialize;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use tauri::AppHandle;

pub const VR_REALMS_APP_ID: u32 = 2964550;
/// Where the kit looks for SteamCMD when nothing else is set.
pub const DEFAULT_STEAMCMD: &str = r"C:\steamcmd\steamcmd.exe";
/// Valve's own download for SteamCMD (developer.valvesoftware.com/wiki/SteamCMD).
const STEAMCMD_ZIP_URL: &str = "https://steamcdn-a.akamaihd.net/client/installer/steamcmd.zip";
const VALVE_SIGNER: &str = "Valve Corp.";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamStatus {
    pub installed: bool,
    pub running: bool,
    pub signed_in: bool,
    pub vr_realms_installed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamCmdStatus {
    pub installed: bool,
    pub path: String,
}

#[cfg(windows)]
fn steam_registry() -> Option<(String, u32)> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let steam = hkcu.open_subkey(r"Software\Valve\Steam").ok()?;
    let path: String = steam.get_value("SteamPath").ok()?;
    let user: u32 = hkcu
        .open_subkey(r"Software\Valve\Steam\ActiveProcess")
        .ok()
        .and_then(|k| k.get_value("ActiveUser").ok())
        .unwrap_or(0);
    Some((path, user))
}

#[cfg(not(windows))]
fn steam_registry() -> Option<(String, u32)> {
    None
}

/// Every Steam library folder, read from `steamapps/libraryfolders.vdf`.
fn library_folders(steam_path: &Path) -> Vec<PathBuf> {
    let mut out = vec![steam_path.to_path_buf()];
    if let Ok(text) = fs::read_to_string(steam_path.join(r"steamapps\libraryfolders.vdf")) {
        for line in text.lines() {
            let parts: Vec<&str> = line.split('"').collect();
            // A line like:   "path"		"D:\\SteamLibrary"
            if parts.len() >= 4 && parts[1] == "path" {
                out.push(PathBuf::from(parts[3].replace("\\\\", "\\")));
            }
        }
    }
    out
}

pub fn status() -> SteamStatus {
    let Some((path, user)) = steam_registry() else {
        return SteamStatus { installed: false, running: false, signed_in: false, vr_realms_installed: false };
    };
    let steam_path = PathBuf::from(path);
    let manifest = format!("appmanifest_{VR_REALMS_APP_ID}.acf");
    let vr_realms_installed = library_folders(&steam_path)
        .iter()
        .any(|lib| lib.join("steamapps").join(&manifest).is_file());
    let running = winutil::is_process_running("steam.exe");
    SteamStatus {
        installed: steam_path.join("steam.exe").is_file(),
        running,
        signed_in: running && user != 0,
        vr_realms_installed,
    }
}

pub fn steamcmd_path(configured: Option<&str>) -> PathBuf {
    match configured.map(str::trim).filter(|s| !s.is_empty()) {
        Some(p) => PathBuf::from(p),
        None => PathBuf::from(DEFAULT_STEAMCMD),
    }
}

pub fn steamcmd_status(configured: Option<&str>) -> SteamCmdStatus {
    let path = steamcmd_path(configured);
    SteamCmdStatus { installed: path.is_file(), path: path.to_string_lossy().into_owned() }
}

/// Downloads SteamCMD from Valve, checks Valve's signature, puts it in `dir`, and lets it update
/// itself once (its first start fetches the rest of SteamCMD; that takes a minute or two).
pub async fn install_steamcmd(app: AppHandle, dir: PathBuf, download_dir: PathBuf) -> AppResult<String> {
    let zip_path = download_dir.join("steamcmd.zip");
    net::download(&app, "steamcmd", "Downloading SteamCMD from Valve", STEAMCMD_ZIP_URL, &zip_path, None).await?;

    let exe = dir.join("steamcmd.exe");
    let zip_for_thread = zip_path.clone();
    let exe_for_thread = exe.clone();
    tauri::async_runtime::spawn_blocking(move || -> AppResult<()> {
        // The zip holds one file, steamcmd.exe. Only that name is taken from it.
        let mut archive = zip::ZipArchive::new(fs::File::open(&zip_for_thread)?)?;
        let mut entry = match archive.by_name("steamcmd.exe") {
            Ok(e) => e,
            Err(_) => return fail("Valve's SteamCMD download did not contain steamcmd.exe."),
        };
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        fs::create_dir_all(exe_for_thread.parent().unwrap_or(Path::new(".")))?;
        let tmp = exe_for_thread.with_extension("exe.new");
        fs::write(&tmp, &bytes)?;
        if let Err(e) = winutil::require_signed_by(&tmp, VALVE_SIGNER) {
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }
        if exe_for_thread.exists() {
            fs::remove_file(&exe_for_thread)?;
        }
        fs::rename(&tmp, &exe_for_thread)?;
        Ok(())
    })
    .await
    .map_err(|e| crate::error::AppError::Message(e.to_string()))??;
    let _ = fs::remove_file(&zip_path);
    applog::info(&format!("SteamCMD (signed by Valve) placed at {}", exe.display()));

    net::emit_progress(&app, "steamcmd", "SteamCMD is updating itself (first start, about a minute)", 0, None);
    let exe_for_update = exe.clone();
    let code = tauri::async_runtime::spawn_blocking(move || {
        winutil::hidden(&exe_for_update)
            .arg("+quit")
            .current_dir(exe_for_update.parent().unwrap_or(Path::new(".")))
            .status()
            .ok()
            .and_then(|s| s.code())
    })
    .await
    .ok()
    .flatten();
    applog::info(&format!("SteamCMD first start finished (code {code:?})"));
    Ok(exe.to_string_lossy().into_owned())
}

/// A Steam account name: letters, digits and underscores, as Steam allows them.
pub fn is_valid_account_name(name: &str) -> bool {
    (2..=64).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Opens SteamCMD's own window signing in `user`. The creator types the password and the Steam Guard
/// code there; SteamCMD remembers the sign-in for later uploads. Typing `quit` closes the window.
pub fn open_login_window(steamcmd: &Path, user: &str) -> AppResult<()> {
    if !is_valid_account_name(user) {
        return fail("That is not a Steam account name. Use the name you sign in to Steam with (not your email and not your profile name).");
    }
    if !steamcmd.is_file() {
        return fail("Install SteamCMD first.");
    }
    let named_right = steamcmd
        .file_name()
        .map(|n| n.to_string_lossy().eq_ignore_ascii_case("steamcmd.exe"))
        .unwrap_or(false);
    if !named_right {
        return fail("The SteamCMD path in the settings does not point at steamcmd.exe.");
    }
    // Only Valve's own SteamCMD gets the account name and the sign-in.
    winutil::require_signed_by(steamcmd, VALVE_SIGNER)?;
    let mut cmd = std::process::Command::new(steamcmd);
    cmd.args(["+login", user]).current_dir(steamcmd.parent().unwrap_or(Path::new(".")));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(winutil::CREATE_NEW_CONSOLE);
    }
    cmd.spawn()?;
    applog::info(&format!("opened SteamCMD's sign-in window for '{user}'"));
    Ok(())
}

/// Puts the SteamCMD path and Steam account name into a kit's own settings, so its Workshop panel
/// has them ready. Only those two lines change; everything else in the file is kept as it was.
pub fn write_kit_settings(kit_root: &Path, steamcmd: &str, user: Option<&str>) -> AppResult<()> {
    if winutil::is_process_running("UnrealEditor.exe") {
        return fail("Close Unreal Editor first: it rewrites this settings file when it closes.");
    }
    let ini = kit_root.join(r"VRRealms\Saved\Config\WindowsEditor\EditorPerProjectUserSettings.ini");
    let mut pairs = vec![("SteamCmdPath", steamcmd.to_string())];
    if let Some(u) = user.filter(|u| is_valid_account_name(u)) {
        pairs.push(("SteamUser", u.to_string()));
    }
    crate::ini::upsert(&ini, "VRRealmsCreatorKit", &pairs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_names() {
        assert!(is_valid_account_name("crazy_sketch85"));
        assert!(!is_valid_account_name("me@mail.com"));
        assert!(!is_valid_account_name("a b"));
        assert!(!is_valid_account_name("x\" +quit"));
        assert!(!is_valid_account_name("x"));
    }

    #[test]
    fn reads_library_folders() {
        let dir = tempfile::tempdir().unwrap();
        let steamapps = dir.path().join("steamapps");
        fs::create_dir_all(&steamapps).unwrap();
        fs::write(
            steamapps.join("libraryfolders.vdf"),
            "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"D:\\\\SteamLibrary\"\n\t}\n}\n",
        )
        .unwrap();
        let libs = library_folders(dir.path());
        assert_eq!(libs.len(), 2);
        assert_eq!(libs[1], PathBuf::from(r"D:\SteamLibrary"));
    }
}
