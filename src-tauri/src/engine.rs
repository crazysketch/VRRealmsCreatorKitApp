//! Finding Unreal Engine 5.8 and opening a project with it.

use crate::error::{fail, AppResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const REQUIRED_MAJOR: u32 = 5;
pub const REQUIRED_MINOR: u32 = 8;
const EPIC_INSTALL_LIST: &str = r"C:\ProgramData\Epic\UnrealEngineLauncher\LauncherInstalled.dat";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    /// "ok" | "wrong_version" | "missing"
    pub state: String,
    pub path: Option<String>,
    pub version: Option<String>,
    pub editor_exe: Option<String>,
    /// Where it was found: "your setting", "Epic Games Launcher", "Windows registry", "folder search".
    pub source: Option<String>,
}

impl EngineStatus {
    fn missing() -> Self {
        Self { state: "missing".into(), path: None, version: None, editor_exe: None, source: None }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct BuildVersion {
    major_version: u32,
    minor_version: u32,
    patch_version: Option<u32>,
}

/// Checks one engine folder (the one holding `Engine\`). Returns its status, or None when it is no engine.
pub fn inspect(dir: &Path, source: &str) -> Option<EngineStatus> {
    let editor = dir.join(r"Engine\Binaries\Win64\UnrealEditor.exe");
    if !editor.is_file() {
        return None;
    }
    let version = fs::read_to_string(dir.join(r"Engine\Build\Build.version"))
        .ok()
        .and_then(|t| serde_json::from_str::<BuildVersion>(&t).ok());
    let (state, text) = match &version {
        Some(v) if v.major_version == REQUIRED_MAJOR && v.minor_version == REQUIRED_MINOR => {
            ("ok", format!("{}.{}.{}", v.major_version, v.minor_version, v.patch_version.unwrap_or(0)))
        }
        Some(v) => ("wrong_version", format!("{}.{}", v.major_version, v.minor_version)),
        None => ("wrong_version", "unknown".to_string()),
    };
    Some(EngineStatus {
        state: state.into(),
        path: Some(dir.to_string_lossy().into_owned()),
        version: Some(text),
        editor_exe: Some(editor.to_string_lossy().into_owned()),
        source: Some(source.into()),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct InstallList {
    installation_list: Vec<InstallEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct InstallEntry {
    install_location: String,
    app_name: String,
}

/// The Epic Games Launcher writes every engine it installed into LauncherInstalled.dat.
/// Only the entry named exactly "UE_5.8" is the engine; plugins (Fab, Bridge) carry "_5.8" too.
fn from_epic_list() -> Option<PathBuf> {
    let text = fs::read_to_string(EPIC_INSTALL_LIST).ok()?;
    let list: InstallList = serde_json::from_str(&text).ok()?;
    let want = format!("UE_{REQUIRED_MAJOR}.{REQUIRED_MINOR}");
    list.installation_list
        .into_iter()
        .find(|e| e.app_name.eq_ignore_ascii_case(&want))
        .map(|e| PathBuf::from(e.install_location))
}

#[cfg(windows)]
fn from_registry() -> Option<PathBuf> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;
    let key = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(format!(r"SOFTWARE\EpicGames\Unreal Engine\{REQUIRED_MAJOR}.{REQUIRED_MINOR}"))
        .ok()?;
    let dir: String = key.get_value("InstalledDirectory").ok()?;
    Some(PathBuf::from(dir))
}

#[cfg(not(windows))]
fn from_registry() -> Option<PathBuf> {
    None
}

fn from_folders() -> Option<PathBuf> {
    let name = format!("UE_{REQUIRED_MAJOR}.{REQUIRED_MINOR}");
    let mut roots = Vec::new();
    if let Ok(pf) = std::env::var("ProgramFiles") {
        roots.push(PathBuf::from(pf).join("Epic Games"));
    }
    for drive in 'C'..='H' {
        roots.push(PathBuf::from(format!(r"{drive}:\Program Files\Epic Games")));
        roots.push(PathBuf::from(format!(r"{drive}:\Epic Games")));
        roots.push(PathBuf::from(format!(r"{drive}:\")));
    }
    roots.into_iter().map(|r| r.join(&name)).find(|p| p.join(r"Engine\Binaries\Win64\UnrealEditor.exe").is_file())
}

/// Finds Unreal Engine 5.8: the folder chosen in Settings first, then Epic's own install list,
/// then the registry, then the usual folders.
pub fn detect(user_path: Option<&str>) -> EngineStatus {
    if let Some(p) = user_path.filter(|p| !p.trim().is_empty()) {
        let mut dir = PathBuf::from(p.trim());
        // Someone may have picked UnrealEditor.exe or a folder inside the engine: walk up to the engine root.
        for _ in 0..5 {
            if let Some(s) = inspect(&dir, "your setting") {
                return s;
            }
            match dir.parent() {
                Some(parent) => dir = parent.to_path_buf(),
                None => break,
            }
        }
    }
    let found = [
        (from_epic_list(), "Epic Games Launcher"),
        (from_registry(), "Windows registry"),
        (from_folders(), "folder search"),
    ];
    for (dir, source) in found {
        if let Some(s) = dir.as_deref().and_then(|d| inspect(d, source)) {
            return s;
        }
    }
    EngineStatus::missing()
}

/// Opens a .uproject in the given Unreal Editor. The two paths go to the program as separate
/// arguments, never through a command line that could be read as something else.
pub fn open_project(editor_exe: &str, uproject: &Path) -> AppResult<()> {
    let editor = Path::new(editor_exe);
    let named_right = editor
        .file_name()
        .map(|n| n.to_string_lossy().eq_ignore_ascii_case("UnrealEditor.exe"))
        .unwrap_or(false);
    if !named_right || !editor.is_file() {
        return fail(format!("Unreal Editor was not found at {editor_exe}. Check Settings."));
    }
    if !uproject.is_file() {
        return fail(format!("The project file {} is not there any more.", uproject.display()));
    }
    std::process::Command::new(editor).arg(uproject).spawn()?;
    crate::applog::info(&format!("opened {} with {}", uproject.display(), editor_exe));
    Ok(())
}
