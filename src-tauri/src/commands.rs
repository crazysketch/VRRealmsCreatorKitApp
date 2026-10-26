//! What the window can ask for. Each command is small: the work lives in the modules.

use crate::config::{self, AppConfig};
use crate::error::{fail, AppResult};
use crate::{applog, engine, epic, kit, news, steam, winutil};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

fn data_dir(app: &AppHandle) -> AppResult<PathBuf> {
    app.path()
        .app_data_dir()
        .map_err(|e| crate::error::AppError::Message(format!("No folder for the app's settings: {e}")))
}

fn download_dir() -> PathBuf {
    std::env::temp_dir().join("VRRealmsCreatorKit")
}

fn load(app: &AppHandle) -> AppResult<AppConfig> {
    config::load(&data_dir(app)?)
}

fn store(app: &AppHandle, cfg: &AppConfig) -> AppResult<()> {
    config::save(&data_dir(app)?, cfg)
}

fn same_path(a: &str, b: &str) -> bool {
    a.trim_end_matches('\\').eq_ignore_ascii_case(b.trim_end_matches('\\'))
}

fn remember_kit(cfg: &mut AppConfig, root: &str, make_current: bool) {
    if !cfg.kits.iter().any(|k| same_path(k, root)) {
        cfg.kits.push(root.to_string());
    }
    if make_current || cfg.current_kit.is_none() {
        cfg.current_kit = Some(root.to_string());
    }
}

// ----- settings --------------------------------------------------------------------------------

#[tauri::command]
pub fn get_config(app: AppHandle) -> AppResult<AppConfig> {
    load(&app)
}

/// Changes the app's own preferences. Paths, kits and the Steam name have commands of their own,
/// each with its own checks, so the window cannot set them here.
#[tauri::command]
pub fn set_preferences(app: AppHandle, check_updates_on_start: Option<bool>, language: Option<String>) -> AppResult<AppConfig> {
    let mut cfg = load(&app)?;
    if let Some(on) = check_updates_on_start {
        cfg.check_updates_on_start = on;
    }
    if let Some(lang) = language {
        let lang = lang.trim();
        if lang.is_empty() || lang.len() > 8 || !lang.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return fail("That is not a language code.");
        }
        cfg.language = lang.to_string();
    }
    store(&app, &cfg)?;
    Ok(cfg)
}

// ----- the "What you need" checklist ------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub engine: engine::EngineStatus,
    pub epic: epic::LauncherStatus,
    pub steam: steam::SteamStatus,
    pub steamcmd: steam::SteamCmdStatus,
    pub kit: Option<kit::KitInfo>,
    pub editor_running: bool,
    /// Free space on the drive Unreal Engine goes to (or C:), in bytes.
    pub free_bytes: Option<u64>,
}

#[tauri::command]
pub async fn get_overview(app: AppHandle) -> AppResult<Overview> {
    let cfg = load(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let engine = engine::detect(cfg.engine_path.as_deref());
        let space_on = engine.path.clone().unwrap_or_else(|| r"C:\".into());
        Overview {
            free_bytes: winutil::free_bytes(Path::new(&space_on)),
            engine,
            epic: epic::status(),
            steam: steam::status(),
            steamcmd: steam::steamcmd_status(cfg.steamcmd_path.as_deref()),
            kit: cfg.current_kit.as_deref().map(|k| kit::read_kit(Path::new(k))),
            editor_running: winutil::is_process_running("UnrealEditor.exe"),
        }
    })
    .await
    .map_err(|e| crate::error::AppError::Message(e.to_string()))
}

// ----- Unreal Engine and Epic -----------------------------------------------------------------

#[tauri::command]
pub fn set_engine_path(app: AppHandle, path: Option<String>) -> AppResult<engine::EngineStatus> {
    let mut cfg = load(&app)?;
    let status = engine::detect(path.as_deref());
    if path.is_some() && status.source.as_deref() != Some("your setting") {
        return fail("That folder is not an Unreal Engine install. Pick the folder that holds 'Engine' (for example C:\\Program Files\\Epic Games\\UE_5.8).");
    }
    cfg.engine_path = path.filter(|p| !p.trim().is_empty());
    store(&app, &cfg)?;
    Ok(status)
}

#[tauri::command]
pub async fn install_epic_launcher(app: AppHandle) -> AppResult<()> {
    epic::install(app, download_dir()).await
}

#[tauri::command]
pub fn open_unreal_page(app: AppHandle) -> AppResult<()> {
    epic::open_unreal_page(&app)
}

// ----- Steam and SteamCMD ----------------------------------------------------------------------

#[tauri::command]
pub async fn install_steamcmd(app: AppHandle, dir: Option<String>) -> AppResult<String> {
    let target = dir
        .filter(|d| !d.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(steam::DEFAULT_STEAMCMD).parent().unwrap().to_path_buf());
    let exe = steam::install_steamcmd(app.clone(), target, download_dir()).await?;
    let mut cfg = load(&app)?;
    cfg.steamcmd_path = if same_path(&exe, steam::DEFAULT_STEAMCMD) { None } else { Some(exe.clone()) };
    store(&app, &cfg)?;
    Ok(exe)
}

/// Saves the Steam account name, hands it and SteamCMD's path to every kit this app knows,
/// and opens SteamCMD's own sign-in window.
#[tauri::command]
pub fn steam_login(app: AppHandle, user: String) -> AppResult<String> {
    let user = user.trim().to_string();
    if !steam::is_valid_account_name(&user) {
        return fail("That is not a Steam account name. Use the name you sign in to Steam with (not your email and not your profile name).");
    }
    let mut cfg = load(&app)?;
    let steamcmd = steam::steamcmd_path(cfg.steamcmd_path.as_deref());
    // The window first: it checks that SteamCMD is Valve's before anything is saved or handed on.
    steam::open_login_window(&steamcmd, &user)?;
    cfg.steam_user = Some(user.clone());
    store(&app, &cfg)?;
    let mut told = 0;
    let mut notes = Vec::new();
    for root in &cfg.kits {
        match steam::write_kit_settings(Path::new(root), &steamcmd.to_string_lossy(), Some(&user)) {
            Ok(()) => told += 1,
            Err(e) => notes.push(format!("{root}: {e}")),
        }
    }
    let mut msg = format!(
        "SteamCMD's window is open. Type your Steam password and your Steam Guard code there (this app never sees them), then type quit. {told} kit(s) now know your Steam name."
    );
    if !notes.is_empty() {
        msg.push_str(&format!(" Not changed: {}", notes.join("; ")));
    }
    Ok(msg)
}

// ----- kits -----------------------------------------------------------------------------------

#[tauri::command]
pub fn list_kits(app: AppHandle) -> AppResult<Vec<kit::KitInfo>> {
    Ok(load(&app)?.kits.iter().map(|k| kit::read_kit(Path::new(k))).collect())
}

#[tauri::command]
pub fn add_kit(app: AppHandle, path: String) -> AppResult<kit::KitInfo> {
    let Some(root) = kit::find_root(Path::new(&path)) else {
        return fail("No Creator Kit was found there. Pick the kit's folder (the one that holds 'VRRealms').");
    };
    let root_s = root.to_string_lossy().into_owned();
    let mut cfg = load(&app)?;
    remember_kit(&mut cfg, &root_s, false);
    store(&app, &cfg)?;
    applog::info(&format!("added the kit at {root_s}"));
    Ok(kit::read_kit(&root))
}

/// Takes a kit off the list. Its files stay where they are.
#[tauri::command]
pub fn forget_kit(app: AppHandle, path: String) -> AppResult<()> {
    let mut cfg = load(&app)?;
    cfg.kits.retain(|k| !same_path(k, &path));
    if cfg.current_kit.as_deref().map(|c| same_path(c, &path)).unwrap_or(false) {
        cfg.current_kit = cfg.kits.first().cloned();
    }
    store(&app, &cfg)
}

#[tauri::command]
pub fn set_current_kit(app: AppHandle, path: String) -> AppResult<()> {
    let mut cfg = load(&app)?;
    remember_kit(&mut cfg, &path, true);
    store(&app, &cfg)
}

#[tauri::command]
pub async fn discover_kits() -> AppResult<Vec<String>> {
    tauri::async_runtime::spawn_blocking(kit::discover)
        .await
        .map_err(|e| crate::error::AppError::Message(e.to_string()))
}

#[tauri::command]
pub fn suggest_kit_folder(parent: Option<String>) -> String {
    let parent = parent.filter(|p| !p.trim().is_empty()).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\"));
    kit::suggest_folder(&parent).to_string_lossy().into_owned()
}

#[tauri::command]
pub async fn install_kit(app: AppHandle, dest: String) -> AppResult<kit::KitInfo> {
    let dest_path = PathBuf::from(dest.trim());
    if !dest_path.is_absolute() {
        return fail("Pick a full folder path, for example C:\\VRRealmsCreatorKit.");
    }
    let info = kit::install_new(app.clone(), dest_path.clone(), download_dir()).await?;
    let mut cfg = load(&app)?;
    remember_kit(&mut cfg, &info.root, true);
    store(&app, &cfg)?;
    // Hand the new kit SteamCMD's path and the Steam name, when they are known.
    let steamcmd = steam::steamcmd_path(cfg.steamcmd_path.as_deref());
    if steamcmd.is_file() || cfg.steam_user.is_some() {
        if let Err(e) = steam::write_kit_settings(&dest_path, &steamcmd.to_string_lossy(), cfg.steam_user.as_deref()) {
            applog::warn(&format!("could not hand the new kit its Steam settings: {e}"));
        }
    }
    Ok(info)
}

#[tauri::command]
pub async fn check_kit_update(path: String) -> AppResult<kit::UpdateCheck> {
    kit::check(PathBuf::from(path)).await
}

#[tauri::command]
pub async fn update_kit(app: AppHandle, path: String, dry_run: bool) -> AppResult<kit::UpdateResult> {
    kit::update(app, PathBuf::from(path), download_dir(), dry_run).await
}

#[tauri::command]
pub async fn verify_kit(path: String) -> AppResult<kit::VerifyResult> {
    kit::verify(PathBuf::from(path)).await
}

#[tauri::command]
pub async fn repair_kit(app: AppHandle, path: String, files: Vec<String>) -> AppResult<String> {
    kit::repair(app, PathBuf::from(path), files, download_dir()).await
}

/// Opens the kit's project in Unreal Engine 5.8.
#[tauri::command]
pub fn open_kit(app: AppHandle, path: Option<String>) -> AppResult<()> {
    let cfg = load(&app)?;
    let Some(root) = path.or(cfg.current_kit.clone()) else {
        return fail("Install the Creator Kit first, or add the one you have.");
    };
    let info = kit::read_kit(Path::new(&root));
    if !info.exists {
        return fail(format!("No Creator Kit was found in {root}."));
    }
    let eng = engine::detect(cfg.engine_path.as_deref());
    match (eng.state.as_str(), eng.editor_exe) {
        ("ok", Some(exe)) => engine::open_project(&exe, Path::new(&info.uproject)),
        ("wrong_version", _) => fail(format!(
            "The Unreal Engine found is version {}, and the kit needs 5.8. Install 5.8 from the Epic Games Launcher, or pick it in Settings.",
            eng.version.unwrap_or_default()
        )),
        _ => fail("Unreal Engine 5.8 was not found. Install it from the Epic Games Launcher first."),
    }
}

/// Shows a folder (or the folder holding a file) in Windows Explorer.
#[tauri::command]
pub fn show_in_explorer(path: String) -> AppResult<()> {
    // A Windows path never holds a quote, so one here is not a path.
    if path.contains('"') || !Path::new(&path).is_absolute() {
        return fail("That is not a folder.");
    }
    let p = Path::new(&path);
    if !p.exists() {
        return fail(format!("{path} is not there."));
    }
    let mut cmd = std::process::Command::new("explorer");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        if p.is_file() {
            cmd.raw_arg(format!("/select,\"{path}\""));
        } else {
            cmd.raw_arg(format!("\"{path}\""));
        }
    }
    #[cfg(not(windows))]
    cmd.arg(p);
    cmd.spawn()?;
    Ok(())
}

// ----- news, log, about ------------------------------------------------------------------------

#[tauri::command]
pub async fn get_news() -> AppResult<news::News> {
    news::fetch().await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogView {
    pub lines: Vec<String>,
    pub file: Option<String>,
}

#[tauri::command]
pub fn get_log() -> LogView {
    LogView { lines: applog::recent(), file: applog::path().map(|p| p.to_string_lossy().into_owned()) }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
}

#[tauri::command]
pub fn app_info(app: AppHandle) -> AppResult<AppInfo> {
    Ok(AppInfo { version: env!("CARGO_PKG_VERSION").into(), data_dir: data_dir(&app)?.to_string_lossy().into_owned() })
}
