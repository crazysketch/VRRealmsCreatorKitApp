//! Small Windows helpers: running processes, free disk space, digital signatures, hidden child processes.

use crate::error::{fail, AppResult};
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// No console window for helper processes the creator never needs to see.
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// A console window of its own (SteamCMD's Steam login needs one to type into).
pub const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

/// A `Command` that runs without flashing a console window.
pub fn hidden(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// True while a process with this executable name runs (case does not matter), e.g. "UnrealEditor.exe".
#[cfg(windows)]
pub fn is_process_running(exe_name: &str) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    let want = exe_name.to_ascii_lowercase();
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return false;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut found = false;
        if Process32FirstW(snap, &mut entry) != 0 {
            loop {
                let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]).to_ascii_lowercase();
                if name == want {
                    found = true;
                    break;
                }
                if Process32NextW(snap, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snap);
        found
    }
}

#[cfg(not(windows))]
pub fn is_process_running(_exe_name: &str) -> bool {
    false
}

/// Free bytes on the drive that holds `path` (the nearest folder that exists is asked).
#[cfg(windows)]
pub fn free_bytes(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let mut probe = path.to_path_buf();
    while !probe.exists() {
        probe = probe.parent()?.to_path_buf();
    }
    let wide: Vec<u16> = probe.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let mut free: u64 = 0;
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, std::ptr::null_mut(), std::ptr::null_mut()) };
    if ok != 0 {
        Some(free)
    } else {
        None
    }
}

#[cfg(not(windows))]
pub fn free_bytes(_path: &Path) -> Option<u64> {
    None
}

#[derive(Deserialize)]
struct SigReport {
    #[serde(rename = "Status")]
    status: String,
    #[serde(rename = "Subject")]
    subject: Option<String>,
}

/// The organization (O=) in a certificate subject such as `CN=Epic Games Inc., O=Epic Games Inc., C=US`.
fn organization(subject: &str) -> Option<String> {
    for part in split_dn(subject) {
        if let Some(v) = part.trim().strip_prefix("O=") {
            return Some(v.trim().trim_matches('"').to_string());
        }
    }
    None
}

/// Splits a distinguished name at commas that are not inside quotes.
fn split_dn(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in s.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                cur.push(c);
            }
            ',' if !quoted => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// Fails unless Windows says `file` carries a valid digital signature from `organization`
/// (for example "Epic Games Inc." or "Valve Corp."). Windows does the checking; the path goes to it
/// through an environment variable, never through the command text.
pub fn require_signed_by(file: &Path, org: &str) -> AppResult<()> {
    let script = "$s = Get-AuthenticodeSignature -LiteralPath $env:VRR_SIGFILE; \
                  [pscustomobject]@{ Status = [string]$s.Status; Subject = [string]$s.SignerCertificate.Subject } \
                  | ConvertTo-Json -Compress";
    let out = hidden("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("VRR_SIGFILE", file)
        .output()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let report: SigReport = match serde_json::from_str(text.trim()) {
        Ok(r) => r,
        Err(_) => return fail("Windows could not check the download's signature, so it was not started."),
    };
    let got = report.subject.as_deref().and_then(organization).unwrap_or_default();
    if report.status != "Valid" || got != org {
        return fail(format!(
            "The download is not signed by {org} (Windows says: {}, signed by '{}'). It was not started.",
            report.status,
            if got.is_empty() { "nobody" } else { &got }
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_organization_from_a_subject() {
        let s = "CN=Epic Games Inc., O=Epic Games Inc., L=Cary, S=North Carolina, C=US";
        assert_eq!(organization(s).as_deref(), Some("Epic Games Inc."));
        let q = "CN=DigiCert Trusted G4, O=\"DigiCert, Inc.\", C=US";
        assert_eq!(organization(q).as_deref(), Some("DigiCert, Inc."));
        assert_eq!(organization("CN=nobody"), None);
    }
}
