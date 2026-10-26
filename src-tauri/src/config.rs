//! Settings, kept as `config.json` in the app's data folder.

use crate::error::AppResult;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const CONFIG_VERSION: u32 = 1;
const CONFIG_FILENAME: &str = "config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub version: u32,
    pub language: String,
    /// Kit folders this app knows about (each holds `VRRealms/VRRealms.uproject`).
    pub kits: Vec<String>,
    /// The kit that Open Creator Kit opens.
    pub current_kit: Option<String>,
    /// Unreal Engine 5.8 folder chosen by hand; empty = found automatically.
    pub engine_path: Option<String>,
    /// SteamCMD chosen by hand; empty = C:\steamcmd\steamcmd.exe, where the kit looks by default.
    pub steamcmd_path: Option<String>,
    /// The Steam account name used for Workshop uploads (never a password).
    pub steam_user: Option<String>,
    pub check_updates_on_start: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            language: "en".into(),
            kits: Vec::new(),
            current_kit: None,
            engine_path: None,
            steamcmd_path: None,
            steam_user: None,
            check_updates_on_start: true,
        }
    }
}

pub fn load(data_dir: &Path) -> AppResult<AppConfig> {
    let path = data_dir.join(CONFIG_FILENAME);
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let text = fs::read_to_string(&path)?;
    // A damaged file must not stop the app from opening: start from the defaults and say so in the log.
    match serde_json::from_str::<AppConfig>(&text) {
        Ok(mut cfg) => {
            cfg.version = CONFIG_VERSION;
            Ok(cfg)
        }
        Err(e) => {
            crate::applog::warn(&format!("config.json could not be read ({e}); using the defaults"));
            let _ = fs::rename(&path, data_dir.join("config.damaged.json"));
            Ok(AppConfig::default())
        }
    }
}

/// Written to a temporary file first, then moved over the old one, so a crash never leaves half a file.
pub fn save(data_dir: &Path, cfg: &AppConfig) -> AppResult<()> {
    fs::create_dir_all(data_dir)?;
    let path = data_dir.join(CONFIG_FILENAME);
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(cfg)?)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}
