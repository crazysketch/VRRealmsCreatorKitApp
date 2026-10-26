//! Installing, updating, checking and repairing the Creator Kit.
//!
//! Every kit release on GitHub carries `manifest.json`: the plugin version, the full kit zip with its
//! SHA-256, and a table of every file in the kit with its own SHA-256. Everything here works from it:
//!
//! - Install: download the full kit zip, refuse it unless its hash matches, unpack it into a new folder.
//! - Update: compare each file on disk with the table of the release that is installed and the table of
//!   the new one. A file the creator has not changed is replaced; a file they changed is kept and listed;
//!   a new file is added. Nothing is ever deleted.
//! - Verify: compare every shipped file with the installed release's table (OK / MODIFIED / MISSING).
//! - Repair: put back the kit's own copy of chosen files.
//!
//! Rules that hold everywhere: a creator's own maps and avatars (`Content/VRRealms/Community`) are never
//! written, every replaced file is copied into a `_VRRUpdaterBackup_...` folder first, and if a write fails
//! half way every file already written is put back. Nothing runs while Unreal Editor has the kit open.

use crate::error::{fail, AppError, AppResult};
use crate::{applog, net, winutil};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Runtime};

pub const REPO: &str = "crazysketch/VRRealmsCreatorKit";
const ZIP_ROOT: &str = "VRRealmsCreatorKit/";
const UPLUGIN_REL: &str = "VRRealms/Plugins/VRRealmsCreatorKit/VRRealmsCreatorKit.uplugin";
const UPROJECT_REL: &str = "VRRealms/VRRealms.uproject";
const COMMUNITY_REL: &str = "VRRealms/Content/VRRealms/Community/";
const CONTENT_VERSION_FILE: &str = ".kit-content-version";
const BACKUP_PREFIX: &str = "_VRRUpdaterBackup_";

/// One kit job at a time: two jobs writing the same folder would undo each other's backups.
static BUSY: AtomicBool = AtomicBool::new(false);

struct BusyGuard;
impl BusyGuard {
    fn take() -> AppResult<Self> {
        if BUSY.swap(true, Ordering::SeqCst) {
            return fail("The kit is already being installed or updated. Wait for that to finish.");
        }
        Ok(BusyGuard)
    }
}
impl Drop for BusyGuard {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::SeqCst);
    }
}

// ---------------------------------------------------------------------------------------------
// Releases and manifests
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct GhRelease {
    pub tag_name: String,
    pub body: Option<String>,
    pub published_at: Option<String>,
    pub html_url: String,
    pub assets: Vec<GhAsset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GhAsset {
    pub name: String,
    pub browser_download_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileEntry {
    pub sha256: String,
    #[allow(dead_code)]
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub plugin_version: String,
    pub content_version: Option<String>,
    pub full_kit_zip: String,
    pub full_kit_sha256: String,
    #[serde(default)]
    pub files: BTreeMap<String, FileEntry>,
}

pub struct Release {
    pub info: GhRelease,
    pub manifest: Manifest,
}

impl Release {
    pub fn version(&self) -> String {
        self.info.tag_name.trim_start_matches(['v', 'V']).to_string()
    }

    fn full_zip_url(&self) -> AppResult<String> {
        match self.info.assets.iter().find(|a| a.name == self.manifest.full_kit_zip) {
            Some(a) => Ok(a.browser_download_url.clone()),
            None => fail(format!("Release {} does not carry {}.", self.version(), self.manifest.full_kit_zip)),
        }
    }
}

async fn fetch_release(url: &str) -> AppResult<Release> {
    let info: GhRelease = net::get_json(url, 30).await?;
    let Some(asset) = info.assets.iter().find(|a| a.name == "manifest.json") else {
        return fail(format!("Release {} has no manifest.json, so it cannot be installed safely.", info.tag_name));
    };
    let manifest: Manifest = net::get_json(&asset.browser_download_url, 30).await?;
    Ok(Release { info, manifest })
}

pub async fn latest_release() -> AppResult<Release> {
    fetch_release(&format!("https://api.github.com/repos/{REPO}/releases/latest")).await
}

pub async fn release_for(version: &str) -> AppResult<Release> {
    fetch_release(&format!("https://api.github.com/repos/{REPO}/releases/tags/v{version}")).await
}

// ---------------------------------------------------------------------------------------------
// Versions
// ---------------------------------------------------------------------------------------------

fn version_parts(v: &str) -> Vec<u32> {
    v.trim().trim_start_matches(['v', 'V']).split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

/// -1, 0 or 1, comparing numerically: 0.4.10 is newer than 0.4.9.
pub fn compare_versions(a: &str, b: &str) -> i32 {
    let (x, y) = (version_parts(a), version_parts(b));
    for i in 0..x.len().max(y.len()) {
        let (p, q) = (x.get(i).copied().unwrap_or(0), y.get(i).copied().unwrap_or(0));
        if p != q {
            return if p < q { -1 } else { 1 };
        }
    }
    0
}

// ---------------------------------------------------------------------------------------------
// Kits on disk
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KitInfo {
    pub root: String,
    pub name: String,
    pub exists: bool,
    /// The plugin version, or None when it cannot be read.
    pub version: Option<String>,
    /// Kits from before version tracking report "1.0"; they are treated as older than any release.
    pub legacy: bool,
    pub content_version: Option<String>,
    pub uproject: String,
}

fn root_is_kit(root: &Path) -> bool {
    root.join(UPLUGIN_REL).is_file()
}

/// The kit folder for whatever was picked: the kit folder itself, its `VRRealms` project folder,
/// the .uproject, or a folder just above the kit.
pub fn find_root(picked: &Path) -> Option<PathBuf> {
    let mut p = picked.to_path_buf();
    if p.is_file() {
        p = p.parent()?.to_path_buf();
    }
    for _ in 0..4 {
        if root_is_kit(&p) {
            return Some(p);
        }
        match p.parent() {
            Some(parent) => p = parent.to_path_buf(),
            None => break,
        }
    }
    // Look down a little too: someone may pick the folder that holds the kit folder.
    let mut found = Vec::new();
    scan_for_kits(picked, 2, &mut found, &mut 0);
    found.into_iter().next()
}

pub fn read_kit(root: &Path) -> KitInfo {
    let name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut info = KitInfo {
        root: root.to_string_lossy().into_owned(),
        name,
        exists: root_is_kit(root),
        version: None,
        legacy: false,
        content_version: fs::read_to_string(root.join(CONTENT_VERSION_FILE)).ok().map(|s| s.trim().to_string()),
        uproject: root.join(UPROJECT_REL).to_string_lossy().into_owned(),
    };
    if let Ok(text) = fs::read_to_string(root.join(UPLUGIN_REL)) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}')) {
            if let Some(v) = json.get("VersionName").and_then(|v| v.as_str()) {
                info.legacy = v == "1.0";
                info.version = Some(v.to_string());
            }
        }
    }
    info
}

/// The version to compare against releases ("0.0.0" for a pre-tracking kit).
fn comparable_version(k: &KitInfo) -> String {
    if k.legacy {
        "0.0.0".into()
    } else {
        k.version.clone().unwrap_or_else(|| "0.0.0".into())
    }
}

const SKIP_DIRS: &[&str] = &[
    "windows", "program files", "program files (x86)", "programdata", "$recycle.bin", "appdata", "node_modules",
    ".git", "saved", "intermediate", "deriveddatacache", "binaries", "content", "steamapps", "system volume information",
];

fn scan_for_kits(dir: &Path, depth: u32, found: &mut Vec<PathBuf>, visited: &mut u32) {
    *visited += 1;
    if *visited > 20_000 || found.len() >= 50 {
        return;
    }
    if root_is_kit(dir) {
        found.push(dir.to_path_buf());
        return;
    }
    if depth == 0 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        if !ft.is_dir() || ft.is_symlink() {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_ascii_lowercase();
        if name.starts_with(&BACKUP_PREFIX.to_ascii_lowercase()) || name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) {
            continue;
        }
        scan_for_kits(&e.path(), depth - 1, found, visited);
    }
}

/// Looks for kits in the usual places: Desktop, Documents, Downloads and the top of each drive.
pub fn discover() -> Vec<String> {
    let mut roots: Vec<(PathBuf, u32)> = Vec::new();
    if let Some(d) = dirs::desktop_dir() {
        roots.push((d, 4));
    }
    if let Some(d) = dirs::document_dir() {
        roots.push((d, 4));
    }
    if let Some(d) = dirs::download_dir() {
        roots.push((d, 3));
    }
    for drive in 'C'..='H' {
        let d = PathBuf::from(format!(r"{drive}:\"));
        if d.exists() {
            roots.push((d, 2));
        }
    }
    let mut found = Vec::new();
    let mut visited = 0;
    for (root, depth) in roots {
        scan_for_kits(&root, depth, &mut found, &mut visited);
    }
    let mut out: Vec<String> = found.iter().map(|p| p.to_string_lossy().into_owned()).collect();
    out.sort_by_key(|s| s.to_ascii_lowercase());
    out.dedup_by_key(|s| s.to_ascii_lowercase());
    out
}

// ---------------------------------------------------------------------------------------------
// Safety rules
// ---------------------------------------------------------------------------------------------

/// A creator's own maps and avatars live under Community. The kit ships only two marker files there.
pub fn is_creators_area(rel: &str) -> bool {
    rel.starts_with(COMMUNITY_REL) && !rel.contains("/_PUT_YOUR_")
}

/// The path a manifest entry names, refused if it could reach outside the kit folder.
fn target_for(root: &Path, rel: &str) -> AppResult<PathBuf> {
    let ok = !rel.is_empty()
        && !rel.starts_with('/')
        && !rel.contains('\\')
        && !rel.contains(':')
        && rel.split('/').all(|seg| !seg.is_empty() && seg != "." && seg != "..");
    if !ok {
        return fail(format!("The release lists a file outside the kit folder ({rel}); nothing was changed."));
    }
    Ok(root.join(rel.replace('/', "\\")))
}

fn ensure_editor_closed() -> AppResult<()> {
    if winutil::is_process_running("UnrealEditor.exe") {
        return fail("Close Unreal Editor first. The kit's files are locked while it is open.");
    }
    Ok(())
}

fn file_sha(path: &Path) -> Option<String> {
    net::sha256_file(path).ok()
}

// ---------------------------------------------------------------------------------------------
// Update plan
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePlan {
    pub from: String,
    pub to: String,
    /// Kit files the creator has not changed, which the new release changes.
    pub replace: Vec<String>,
    /// Files the new release adds.
    pub add: Vec<String>,
    /// Files the creator changed that the new release also changes: theirs are kept.
    pub kept_yours: Vec<String>,
    /// True when the installed release had no file table, so only the kit's plugin folder is updated.
    pub plugin_folder_only: bool,
}

pub fn plan_update(root: &Path, installed: Option<&Manifest>, new: &Manifest, from: &str, to: &str) -> AppResult<UpdatePlan> {
    let mut plan = UpdatePlan { from: from.into(), to: to.into(), plugin_folder_only: installed.map(|m| m.files.is_empty()).unwrap_or(true), ..Default::default() };
    for (rel, nf) in &new.files {
        if is_creators_area(rel) {
            continue;
        }
        let target = target_for(root, rel)?;
        if !target.exists() {
            plan.add.push(rel.clone());
            continue;
        }
        let local = file_sha(&target).unwrap_or_default();
        if local.eq_ignore_ascii_case(&nf.sha256) {
            continue;
        }
        if plan.plugin_folder_only {
            // No table for the installed release: the plugin folder and the updater are the kit's own,
            // everything else may hold the creator's changes.
            if rel.starts_with("VRRealms/Plugins/VRRealmsCreatorKit/") {
                plan.replace.push(rel.clone());
            } else {
                plan.kept_yours.push(rel.clone());
            }
            continue;
        }
        match installed.and_then(|m| m.files.get(rel)) {
            // The kit did not change this file between the two releases: whatever differs is the creator's.
            Some(old) if old.sha256.eq_ignore_ascii_case(&nf.sha256) => {}
            Some(old) if old.sha256.eq_ignore_ascii_case(&local) => plan.replace.push(rel.clone()),
            Some(_) => plan.kept_yours.push(rel.clone()),
            // A file that was not in the old release but is on disk already: someone put it there.
            None => plan.kept_yours.push(rel.clone()),
        }
    }
    Ok(plan)
}

// ---------------------------------------------------------------------------------------------
// Writing files safely
// ---------------------------------------------------------------------------------------------

fn zip_bytes(archive: &mut zip::ZipArchive<File>, rel: &str) -> AppResult<Vec<u8>> {
    let name = format!("{ZIP_ROOT}{rel}");
    let mut entry = archive
        .by_name(&name)
        .map_err(|_| AppError::Archive(format!("The kit zip does not contain {rel}; nothing was changed.")))?;
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Every existing file in `rels` must open for writing before anything is changed. Returns the ones that do not.
fn refused_targets(root: &Path, rels: &[String]) -> AppResult<Vec<String>> {
    let mut refused = Vec::new();
    for rel in rels {
        let t = target_for(root, rel)?;
        if t.exists() && OpenOptions::new().write(true).open(&t).is_err() {
            refused.push(rel.clone());
        }
    }
    Ok(refused)
}

fn refused_message(refused: &[String]) -> String {
    let more = if refused.len() > 1 { format!(" and {} more", refused.len() - 1) } else { String::new() };
    format!(
        "Windows will not let this app replace '{}'{more}. Nothing was changed. With Unreal Editor closed, this is \
         almost always security software that guards folders such as Desktop and Documents (ransomware protection). \
         Allow this app in it, or keep the kit in a plain folder such as C:\\VRRealmsCreatorKit.",
        refused[0]
    )
}

/// Writes `files` (relative path, new bytes) into the kit. Existing files are copied to `backup_dir`
/// first. If any write fails, every file already written is put back and the error says so.
fn write_files(root: &Path, files: &[(String, Vec<u8>)], backup_dir: &Path) -> AppResult<()> {
    let mut written: Vec<(PathBuf, Option<PathBuf>)> = Vec::new();
    let result: AppResult<()> = (|| {
        for (rel, bytes) in files {
            let target = target_for(root, rel)?;
            let backup = if target.exists() {
                let b = target_for(backup_dir, rel)?;
                fs::create_dir_all(b.parent().unwrap_or(backup_dir))?;
                fs::copy(&target, &b)?;
                Some(b)
            } else {
                None
            };
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            written.push((target.clone(), backup));
            fs::write(&target, bytes)?;
        }
        Ok(())
    })();
    if let Err(e) = result {
        let mut not_back = 0;
        for (target, backup) in written.iter().rev() {
            let restored = match backup {
                Some(b) => fs::copy(b, target).is_ok(),
                None => !target.exists() || fs::remove_file(target).is_ok(),
            };
            if !restored {
                not_back += 1;
            }
        }
        let tail = if not_back == 0 {
            "Every file already replaced was put back, so the kit is as it was.".to_string()
        } else {
            format!("{not_back} file(s) could not be put back; the previous copies are in {}.", backup_dir.display())
        };
        applog::error(&format!("writing the kit failed: {e}; {tail}"));
        return fail(format!("Could not write the kit's files ({e}). {tail}"));
    }
    Ok(())
}

fn backup_dir_for(root: &Path, version: &str, what: &str) -> PathBuf {
    let stamp = chrono::Local::now().format("%m%d-%H%M%S");
    root.join(format!("{BACKUP_PREFIX}{version}{what}_{stamp}"))
}

fn open_zip(path: &Path) -> AppResult<zip::ZipArchive<File>> {
    Ok(zip::ZipArchive::new(File::open(path)?)?)
}

// ---------------------------------------------------------------------------------------------
// Jobs
// ---------------------------------------------------------------------------------------------

/// Downloads the release's full kit zip into `download_dir` and checks it against the manifest.
async fn fetch_full_zip<R: Runtime>(app: &AppHandle<R>, release: &Release, download_dir: &Path) -> AppResult<PathBuf> {
    let url = release.full_zip_url()?;
    let path = download_dir.join(&release.manifest.full_kit_zip);
    let label = format!("Downloading Creator Kit {}", release.version());
    net::download(app, "kit", &label, &url, &path, Some(&release.manifest.full_kit_sha256)).await?;
    Ok(path)
}

/// Installs the latest kit into `dest`, a folder that does not exist yet or is empty.
pub async fn install_new<R: Runtime>(app: AppHandle<R>, dest: PathBuf, download_dir: PathBuf) -> AppResult<KitInfo> {
    let _busy = BusyGuard::take()?;
    if dest.exists() && fs::read_dir(&dest)?.next().is_some() {
        return fail(format!("{} already has files in it. Pick an empty folder or a new folder name.", dest.display()));
    }
    let release = latest_release().await?;
    install_release(&app, &dest, &download_dir, &release).await
}

/// Unpacks one release's full kit into `dest` (which the caller has checked is empty).
async fn install_release<R: Runtime>(app: &AppHandle<R>, dest: &Path, download_dir: &Path, release: &Release) -> AppResult<KitInfo> {
    let dest = dest.to_path_buf();
    let zip_path = fetch_full_zip(app, release, download_dir).await?;
    net::emit_progress(app, "kit", "Unpacking the kit", 0, None);
    let dest_for_thread = dest.clone();
    let manifest = release.manifest.clone();
    tauri::async_runtime::spawn_blocking(move || -> AppResult<()> {
        let mut archive = open_zip(&zip_path)?;
        let rels: Vec<String> = manifest.files.keys().cloned().collect();
        let mut files = Vec::with_capacity(rels.len());
        for rel in rels {
            target_for(&dest_for_thread, &rel)?;
            let bytes = zip_bytes(&mut archive, &rel)?;
            files.push((rel, bytes));
        }
        fs::create_dir_all(&dest_for_thread)?;
        write_files(&dest_for_thread, &files, &dest_for_thread.join("_unused_backup"))?;
        if let Some(cv) = &manifest.content_version {
            fs::write(dest_for_thread.join(CONTENT_VERSION_FILE), cv)?;
        }
        let _ = fs::remove_file(&zip_path);
        Ok(())
    })
    .await
    .map_err(|e| AppError::Message(e.to_string()))??;
    applog::info(&format!("installed Creator Kit {} into {}", release.version(), dest.display()));
    Ok(read_kit(&dest))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    pub installed: Option<String>,
    pub latest: String,
    pub has_update: bool,
    pub notes: String,
    pub published_at: Option<String>,
    pub release_page: String,
}

pub async fn check(root: PathBuf) -> AppResult<UpdateCheck> {
    let kit = read_kit(&root);
    let release = latest_release().await?;
    let latest = release.version();
    let has_update = !kit.exists || compare_versions(&comparable_version(&kit), &latest) < 0;
    Ok(UpdateCheck {
        installed: kit.version.clone(),
        latest,
        has_update,
        notes: release.info.body.clone().unwrap_or_default(),
        published_at: release.info.published_at.clone(),
        release_page: release.info.html_url.clone(),
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResult {
    pub plan: UpdatePlan,
    pub backup_folder: Option<String>,
    pub message: String,
}

/// Brings the kit at `root` up to the latest release. With `dry_run` it only says what it would do.
pub async fn update<R: Runtime>(app: AppHandle<R>, root: PathBuf, download_dir: PathBuf, dry_run: bool) -> AppResult<UpdateResult> {
    let _busy = if dry_run { None } else { Some(BusyGuard::take()?) };
    let kit = read_kit(&root);
    if !kit.exists {
        return fail(format!("No Creator Kit was found in {}.", root.display()));
    }
    let latest = latest_release().await?;
    let from = comparable_version(&kit);
    let to = latest.version();
    if compare_versions(&from, &to) > 0 {
        return fail(format!("This kit ({from}) is newer than the latest release ({to}). Nothing to do."));
    }
    // The installed release's table tells which files the creator changed. Kits from before
    // the file table (or a release that is gone) fall back to updating the plugin folder only.
    let installed = if kit.legacy || kit.version.is_none() {
        None
    } else {
        match release_for(&from).await {
            Ok(r) => Some(r.manifest),
            Err(e) => {
                applog::warn(&format!("no file table for the installed release {from}: {e}"));
                None
            }
        }
    };
    let root_for_plan = root.clone();
    let new_manifest = latest.manifest.clone();
    let (from2, to2) = (from.clone(), to.clone());
    let plan = tauri::async_runtime::spawn_blocking(move || plan_update(&root_for_plan, installed.as_ref(), &new_manifest, &from2, &to2))
        .await
        .map_err(|e| AppError::Message(e.to_string()))??;

    let changes = plan.replace.len() + plan.add.len();
    if dry_run || changes == 0 {
        let message = if changes == 0 {
            if plan.kept_yours.is_empty() {
                format!("Your kit already matches {to}.")
            } else {
                format!("Your kit matches {to}, apart from {} file(s) you changed (kept).", plan.kept_yours.len())
            }
        } else {
            format!("{} file(s) to replace, {} to add, {} of yours kept.", plan.replace.len(), plan.add.len(), plan.kept_yours.len())
        };
        if !dry_run && changes == 0 {
            if let Some(cv) = &latest.manifest.content_version {
                let _ = fs::write(root.join(CONTENT_VERSION_FILE), cv);
            }
        }
        return Ok(UpdateResult { plan, backup_folder: None, message });
    }

    ensure_editor_closed()?;
    let zip_path = fetch_full_zip(&app, &latest, &download_dir).await?;
    net::emit_progress(&app, "kit", "Updating the kit's files", 0, None);
    let root_for_write = root.clone();
    let plan_for_write = plan.clone();
    let content_version = latest.manifest.content_version.clone();
    let backup = backup_dir_for(&root, &from, "");
    let backup_for_write = backup.clone();
    tauri::async_runtime::spawn_blocking(move || -> AppResult<()> {
        ensure_editor_closed()?;
        let rels: Vec<String> = plan_for_write.replace.iter().chain(plan_for_write.add.iter()).cloned().collect();
        let refused = refused_targets(&root_for_write, &rels)?;
        if !refused.is_empty() {
            return fail(refused_message(&refused));
        }
        let mut archive = open_zip(&zip_path)?;
        let mut files = Vec::with_capacity(rels.len());
        for rel in rels {
            let bytes = zip_bytes(&mut archive, &rel)?;
            files.push((rel, bytes));
        }
        write_files(&root_for_write, &files, &backup_for_write)?;
        if let Some(cv) = content_version {
            fs::write(root_for_write.join(CONTENT_VERSION_FILE), cv)?;
        }
        let _ = fs::remove_file(&zip_path);
        Ok(())
    })
    .await
    .map_err(|e| AppError::Message(e.to_string()))??;

    let after = read_kit(&root);
    if after.version.as_deref() != Some(latest.manifest.plugin_version.as_str()) {
        applog::warn(&format!(
            "after the update the kit reports {:?}, the release says {}",
            after.version, latest.manifest.plugin_version
        ));
    }
    applog::info(&format!(
        "updated the kit at {} from {from} to {} ({} replaced, {} added, {} kept)",
        root.display(),
        after.version.as_deref().unwrap_or("?"),
        plan.replace.len(),
        plan.add.len(),
        plan.kept_yours.len()
    ));
    let backup_folder = backup.exists().then(|| backup.to_string_lossy().into_owned());
    let mut message = format!("Updated to {}.", after.version.unwrap_or(to));
    if let Some(b) = &backup_folder {
        message.push_str(&format!(" The files it replaced are kept in {}.", Path::new(b).file_name().unwrap_or_default().to_string_lossy()));
    }
    if !plan.kept_yours.is_empty() {
        message.push_str(&format!(" {} file(s) you had changed were left as yours; Verify shows them.", plan.kept_yours.len()));
    }
    Ok(UpdateResult { plan, backup_folder, message })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyRow {
    pub file: String,
    /// "ok" | "modified" | "missing"
    pub state: String,
    /// Inside Community: a creator's area, never repaired.
    pub yours: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyResult {
    pub version: String,
    pub rows: Vec<VerifyRow>,
    pub ok: usize,
    pub modified: usize,
    pub missing: usize,
}

pub fn verify_against(root: &Path, manifest: &Manifest, version: &str) -> AppResult<VerifyResult> {
    let mut rows = Vec::new();
    for (rel, f) in &manifest.files {
        let target = target_for(root, rel)?;
        let state = if !target.exists() {
            "missing"
        } else if file_sha(&target).map(|h| h.eq_ignore_ascii_case(&f.sha256)).unwrap_or(false) {
            "ok"
        } else {
            "modified"
        };
        rows.push(VerifyRow { file: rel.clone(), state: state.into(), yours: is_creators_area(rel) });
    }
    // Problems first.
    rows.sort_by_key(|r| (r.state == "ok", r.file.clone()));
    let count = |s: &str| rows.iter().filter(|r| r.state == s).count();
    Ok(VerifyResult { version: version.into(), ok: count("ok"), modified: count("modified"), missing: count("missing"), rows })
}

/// Checks every shipped file against the table of the release that is installed.
pub async fn verify(root: PathBuf) -> AppResult<VerifyResult> {
    let kit = read_kit(&root);
    let Some(version) = kit.version.clone().filter(|_| !kit.legacy) else {
        return fail("This kit is too old to check file by file. Update it first, then Verify.");
    };
    let release = release_for(&version).await.map_err(|e| {
        AppError::Message(format!("Verify compares against release {version}, which could not be read from GitHub ({e})."))
    })?;
    if release.manifest.files.is_empty() {
        return fail(format!("Release {version} was published before the kit carried a file table. Update first, then Verify."));
    }
    tauri::async_runtime::spawn_blocking(move || verify_against(&root, &release.manifest, &version))
        .await
        .map_err(|e| AppError::Message(e.to_string()))?
}

/// Puts back the installed release's own copy of `files` (as listed by Verify). The current copies are
/// backed up first; files in a creator's own area are never touched.
pub async fn repair<R: Runtime>(app: AppHandle<R>, root: PathBuf, files: Vec<String>, download_dir: PathBuf) -> AppResult<String> {
    let _busy = BusyGuard::take()?;
    ensure_editor_closed()?;
    let kit = read_kit(&root);
    let Some(version) = kit.version.clone().filter(|_| !kit.legacy) else {
        return fail("This kit is too old to repair file by file. Update it first.");
    };
    let release = release_for(&version).await?;
    let wanted: Vec<String> = files
        .into_iter()
        .filter(|f| release.manifest.files.contains_key(f) && !is_creators_area(f))
        .collect();
    if wanted.is_empty() {
        return Ok("Nothing to repair.".into());
    }
    let zip_path = fetch_full_zip(&app, &release, &download_dir).await?;
    let root_for_write = root.clone();
    let backup = backup_dir_for(&root, &version, "_repair");
    let backup_for_write = backup.clone();
    let count = wanted.len();
    tauri::async_runtime::spawn_blocking(move || -> AppResult<()> {
        ensure_editor_closed()?;
        let refused = refused_targets(&root_for_write, &wanted)?;
        if !refused.is_empty() {
            return fail(refused_message(&refused));
        }
        let mut archive = open_zip(&zip_path)?;
        let mut out = Vec::with_capacity(wanted.len());
        for rel in wanted {
            let bytes = zip_bytes(&mut archive, &rel)?;
            out.push((rel, bytes));
        }
        write_files(&root_for_write, &out, &backup_for_write)?;
        let _ = fs::remove_file(&zip_path);
        Ok(())
    })
    .await
    .map_err(|e| AppError::Message(e.to_string()))??;
    applog::info(&format!("repaired {count} file(s) in {}", root.display()));
    let mut msg = format!("Restored {count} file(s) from release {version}.");
    if backup.exists() {
        msg.push_str(&format!(" Your previous copies are in {}.", backup.file_name().unwrap_or_default().to_string_lossy()));
    }
    Ok(msg)
}

/// A free folder name for a new kit inside `parent`: VRRealmsCreatorKit, VRRealmsCreatorKit2, ...
pub fn suggest_folder(parent: &Path) -> PathBuf {
    let base = parent.join("VRRealmsCreatorKit");
    if !base.exists() {
        return base;
    }
    (2..100).map(|n| parent.join(format!("VRRealmsCreatorKit{n}"))).find(|p| !p.exists()).unwrap_or(base)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(sha: &str) -> FileEntry {
        FileEntry { sha256: sha.into(), size: 1 }
    }

    fn manifest(files: &[(&str, &str)]) -> Manifest {
        Manifest {
            plugin_version: "1".into(),
            content_version: Some("1".into()),
            full_kit_zip: "k.zip".into(),
            full_kit_sha256: "x".into(),
            files: files.iter().map(|(k, v)| (k.to_string(), entry(v))).collect(),
        }
    }

    fn sha_of(text: &str) -> String {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::digest(text.as_bytes()))
    }

    #[test]
    fn versions_compare_as_numbers() {
        assert_eq!(compare_versions("0.4.9", "0.4.10"), -1);
        assert_eq!(compare_versions("v0.4.32", "0.4.32"), 0);
        assert_eq!(compare_versions("0.5", "0.4.99"), 1);
    }

    #[test]
    fn paths_outside_the_kit_are_refused() {
        let root = Path::new(r"C:\kit");
        assert!(target_for(root, "VRRealms/Config/DefaultEngine.ini").is_ok());
        for bad in ["../x", "a/../../x", "/etc/x", "C:/x", r"a\b", "a//b", ""] {
            assert!(target_for(root, bad).is_err(), "{bad} should be refused");
        }
    }

    #[test]
    fn the_creators_area_is_protected() {
        assert!(is_creators_area("VRRealms/Content/VRRealms/Community/Maps/MyMap/MyMap.umap"));
        assert!(!is_creators_area("VRRealms/Content/VRRealms/Community/_PUT_YOUR_MAPS_HERE.txt"));
        assert!(!is_creators_area("VRRealms/Plugins/VRRealmsCreatorKit/VRRealmsCreatorKit.uplugin"));
    }

    #[test]
    fn plan_replaces_untouched_files_and_keeps_changed_ones() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let write = |rel: &str, text: &str| {
            let p = root.join(rel.replace('/', "\\"));
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, text).unwrap();
        };
        write("A.txt", "old a"); // untouched by the creator, changed by the kit -> replace
        write("B.txt", "mine"); // changed by the creator, changed by the kit -> kept
        write("C.txt", "mine c"); // changed by the creator, not changed by the kit -> nothing
        write("D.txt", "same"); // already the new content -> nothing
        write("VRRealms/Content/VRRealms/Community/Maps/M.umap", "creator map"); // creator's area -> never
        let old = manifest(&[
            ("A.txt", &sha_of("old a")),
            ("B.txt", &sha_of("old b")),
            ("C.txt", &sha_of("old c")),
            ("D.txt", &sha_of("old d")),
        ]);
        let new = manifest(&[
            ("A.txt", &sha_of("new a")),
            ("B.txt", &sha_of("new b")),
            ("C.txt", &sha_of("old c")),
            ("D.txt", &sha_of("same")),
            ("E.txt", &sha_of("new e")),
            ("VRRealms/Content/VRRealms/Community/Maps/M.umap", &sha_of("kit map")),
        ]);
        let plan = plan_update(root, Some(&old), &new, "0.4.31", "0.4.32").unwrap();
        assert_eq!(plan.replace, vec!["A.txt"]);
        assert_eq!(plan.add, vec!["E.txt"]);
        assert_eq!(plan.kept_yours, vec!["B.txt"]);
        assert!(!plan.plugin_folder_only);
    }

    #[test]
    fn a_failed_write_puts_everything_back() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("kit");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.txt"), "original").unwrap();
        // The second file's folder is a file, so creating it fails half way.
        fs::write(root.join("blocker"), "x").unwrap();
        let files = vec![("a.txt".to_string(), b"new".to_vec()), ("blocker/b.txt".to_string(), b"new".to_vec())];
        let err = write_files(&root, &files, &root.join("_backup")).unwrap_err().to_string();
        assert!(err.contains("put back"), "{err}");
        assert_eq!(fs::read_to_string(root.join("a.txt")).unwrap(), "original");
    }

    /// The whole road against the real releases on GitHub: an older kit is installed, the creator adds
    /// a map and edits a kit file, the kit is updated, and Verify / Repair are run.
    /// `cargo test -- --ignored` runs it (it needs the internet).
    #[test]
    #[ignore = "downloads the real kit releases from GitHub"]
    fn real_releases_install_update_verify_repair() {
        let app = tauri::test::mock_app();
        let handle = app.handle().clone();
        tauri::async_runtime::block_on(async move {
            let dir = tempfile::tempdir().unwrap();
            let dl = dir.path().join("downloads");
            let root = dir.path().join("Kit");

            let latest = latest_release().await.unwrap();
            let older = latest_release_before(&latest.version()).await;
            install_release(&handle, &root, &dl, &older).await.unwrap();
            assert_eq!(read_kit(&root).version, Some(older.version()), "the older kit is installed");

            // The creator's own map, and a kit file they edited that the new release also changes.
            let map = root.join(r"VRRealms\Content\VRRealms\Community\Maps\Mine\Mine.umap");
            fs::create_dir_all(map.parent().unwrap()).unwrap();
            fs::write(&map, "my map").unwrap();
            let changed: Vec<String> = latest
                .manifest
                .files
                .iter()
                .filter(|(rel, f)| {
                    !rel.starts_with("VRRealms/Plugins/")
                        && older.manifest.files.get(*rel).map(|o| !o.sha256.eq_ignore_ascii_case(&f.sha256)).unwrap_or(false)
                })
                .map(|(r, _)| r.clone())
                .collect();
            assert!(!changed.is_empty(), "the two releases differ outside the plugin folder");
            let edited = changed[0].clone();
            let edited_path = root.join(edited.replace('/', "\\"));
            fs::write(&edited_path, "the creator's own version").unwrap();

            let plan = update(handle.clone(), root.clone(), dl.clone(), true).await.unwrap();
            assert!(plan.plan.kept_yours.contains(&edited), "{:?}", plan.plan);
            assert!(!plan.plan.replace.is_empty());

            let done = update(handle.clone(), root.clone(), dl.clone(), false).await.unwrap();
            assert_eq!(read_kit(&root).version, Some(latest.version()), "updated: {}", done.message);
            assert_eq!(fs::read_to_string(&map).unwrap(), "my map", "the creator's map is untouched");
            assert_eq!(fs::read_to_string(&edited_path).unwrap(), "the creator's own version", "their edit is kept");
            let backup = PathBuf::from(done.backup_folder.expect("replaced files are backed up"));
            assert!(backup.join(r"VRRealms\Plugins\VRRealmsCreatorKit\VRRealmsCreatorKit.uplugin").is_file());

            let v = verify(root.clone()).await.unwrap();
            assert!(v.rows.iter().any(|r| r.file == edited && r.state == "modified"));
            repair(handle.clone(), root.clone(), vec![edited.clone()], dl.clone()).await.unwrap();
            let v2 = verify(root.clone()).await.unwrap();
            assert_eq!(v2.modified + v2.missing, 0, "repair restored the kit's copy");
            assert_eq!(fs::read_to_string(&map).unwrap(), "my map");
        });
    }

    /// The newest release before `version` that carries a file table.
    async fn latest_release_before(version: &str) -> Release {
        let all: Vec<GhRelease> =
            net::get_json(&format!("https://api.github.com/repos/{REPO}/releases?per_page=10"), 30).await.unwrap();
        for r in all {
            let v = r.tag_name.trim_start_matches(['v', 'V']).to_string();
            if compare_versions(&v, version) < 0 {
                if let Ok(rel) = release_for(&v).await {
                    if !rel.manifest.files.is_empty() {
                        return rel;
                    }
                }
            }
        }
        panic!("no older release with a file table");
    }

    #[test]
    fn finds_the_kit_from_anything_inside_it() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("MyKit");
        let up = root.join(UPLUGIN_REL.replace('/', "\\"));
        fs::create_dir_all(up.parent().unwrap()).unwrap();
        fs::write(&up, r#"{"VersionName": "0.4.32"}"#).unwrap();
        fs::write(root.join(UPROJECT_REL.replace('/', "\\")), "{}").unwrap();
        assert_eq!(find_root(&root).unwrap(), root);
        assert_eq!(find_root(&root.join("VRRealms")).unwrap(), root);
        assert_eq!(find_root(&root.join(UPROJECT_REL.replace('/', "\\"))).unwrap(), root);
        assert_eq!(find_root(dir.path()).unwrap(), root);
        let k = read_kit(&root);
        assert_eq!(k.version.as_deref(), Some("0.4.32"));
        assert!(!k.legacy);
    }
}
