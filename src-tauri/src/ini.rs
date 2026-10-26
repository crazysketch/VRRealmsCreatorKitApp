//! Setting a few keys in one section of an Unreal .ini file without disturbing anything else:
//! other sections, other keys, comments, the file's encoding and its line endings stay as they were.

use crate::error::{fail, AppResult};
use std::fs;
use std::path::Path;

enum Encoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
}

fn decode(bytes: &[u8]) -> Option<(String, Encoding)> {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let units: Vec<u16> = bytes[2..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        return String::from_utf16(&units).ok().map(|s| (s, Encoding::Utf16Le));
    }
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8(bytes[3..].to_vec()).ok().map(|s| (s, Encoding::Utf8Bom));
    }
    String::from_utf8(bytes.to_vec()).ok().map(|s| (s, Encoding::Utf8))
}

fn encode(text: &str, enc: &Encoding) -> Vec<u8> {
    match enc {
        Encoding::Utf8 => text.as_bytes().to_vec(),
        Encoding::Utf8Bom => [&[0xEF, 0xBB, 0xBF][..], text.as_bytes()].concat(),
        Encoding::Utf16Le => {
            let mut out = vec![0xFF, 0xFE];
            for u in text.encode_utf16() {
                out.extend_from_slice(&u.to_le_bytes());
            }
            out
        }
    }
}

/// The text with `pairs` set in `[section]`: an existing `Key=` line is replaced, a missing one is
/// added at the end of the section, and a missing section is added at the end of the file.
pub fn upsert_text(text: &str, section: &str, pairs: &[(&str, String)]) -> String {
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let header = format!("[{section}]");
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let start = lines.iter().position(|l| l.trim() == header);

    match start {
        None => {
            while lines.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
                lines.pop();
            }
            if !lines.is_empty() {
                lines.push(String::new());
            }
            lines.push(header);
            for (k, v) in pairs {
                lines.push(format!("{k}={v}"));
            }
        }
        Some(start) => {
            let end = lines[start + 1..]
                .iter()
                .position(|l| l.trim_start().starts_with('['))
                .map(|i| start + 1 + i)
                .unwrap_or(lines.len());
            let mut missing = Vec::new();
            for (k, v) in pairs {
                let prefix = format!("{k}=");
                match (start + 1..end).find(|&i| lines[i].starts_with(&prefix)) {
                    Some(i) => lines[i] = format!("{k}={v}"),
                    None => missing.push(format!("{k}={v}")),
                }
            }
            // New keys go after the section's last non-empty line.
            let mut at = end;
            while at > start + 1 && lines[at - 1].trim().is_empty() {
                at -= 1;
            }
            for (n, line) in missing.into_iter().enumerate() {
                lines.insert(at + n, line);
            }
        }
    }
    let mut out = lines.join(eol);
    out.push_str(eol);
    out
}

pub fn upsert(path: &Path, section: &str, pairs: &[(&str, String)]) -> AppResult<()> {
    let (text, enc) = if path.exists() {
        match decode(&fs::read(path)?) {
            Some(d) => d,
            None => return fail(format!("{} is not in a format this app can safely change; it was left as it is.", path.display())),
        }
    } else {
        (String::new(), Encoding::Utf8)
    };
    let updated = upsert_text(&text, section, pairs);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("ini.tmp");
    fs::write(&tmp, encode(&updated, &enc))?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_and_adds_inside_the_section_only() {
        let src = "[A]\r\nSteamUser=x\r\n\r\n[VRRealmsCreatorKit]\r\nSteamCmdPath=old\r\nbPlayInVR=True\r\n\r\n[B]\r\nSteamUser=keep\r\n";
        let out = upsert_text(src, "VRRealmsCreatorKit", &[("SteamCmdPath", "C:\\steamcmd\\steamcmd.exe".into()), ("SteamUser", "me".into())]);
        assert_eq!(
            out,
            "[A]\r\nSteamUser=x\r\n\r\n[VRRealmsCreatorKit]\r\nSteamCmdPath=C:\\steamcmd\\steamcmd.exe\r\nbPlayInVR=True\r\nSteamUser=me\r\n\r\n[B]\r\nSteamUser=keep\r\n"
        );
    }

    #[test]
    fn adds_a_missing_section() {
        let out = upsert_text("[A]\nX=1\n", "VRRealmsCreatorKit", &[("SteamUser", "me".into())]);
        assert_eq!(out, "[A]\nX=1\n\n[VRRealmsCreatorKit]\nSteamUser=me\n");
        let empty = upsert_text("", "S", &[("K", "v".into())]);
        assert_eq!(empty, "[S]\nK=v\n");
    }

    #[test]
    fn keeps_utf16() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("t.ini");
        fs::write(&p, encode("[S]\r\nK=old\r\n", &Encoding::Utf16Le)).unwrap();
        upsert(&p, "S", &[("K", "new".into())]).unwrap();
        let bytes = fs::read(&p).unwrap();
        assert!(bytes.starts_with(&[0xFF, 0xFE]));
        assert_eq!(decode(&bytes).unwrap().0, "[S]\r\nK=new\r\n");
    }
}
