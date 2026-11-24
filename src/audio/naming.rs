//! Recording folder naming.

use std::io;
use std::path::{Path, PathBuf};

const MAX_NAME_CHARS: usize = 80;
const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Makes `raw` safe as a Windows folder name: invalid and control characters become `_`,
/// leading/trailing spaces and dots go, device names (`CON`, `nul.txt`) get a `_` prefix,
/// empty becomes `recording`, and the result is cut to 80 characters (never mid-character).
pub fn sanitize_name(raw: &str) -> String {
    let replaced: String = raw
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cut: String = replaced.chars().take(MAX_NAME_CHARS).collect();
    let trimmed = cut.trim_matches(|c| c == ' ' || c == '.');
    if trimmed.is_empty() {
        return "recording".to_string();
    }
    let stem = trimmed.split('.').next().unwrap_or("").trim_end();
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
        format!("_{trimmed}")
    } else {
        trimmed.to_string()
    }
}

/// Creates `base` if needed, then the session folder `<stamp>_<sanitized name>`; when that
/// exists it tries `-2`, `-3`, ... (`create_dir` makes the collision check atomic).
pub fn session_dir(base: &Path, stamp: &str, name: &str) -> io::Result<PathBuf> {
    std::fs::create_dir_all(base)?;
    let stem = format!("{stamp}_{}", sanitize_name(name));
    for n in 1..=1000 {
        let leaf = if n == 1 {
            stem.clone()
        } else {
            format!("{stem}-{n}")
        };
        let dir = base.join(leaf);
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::other("too many recordings with the same name"))
}

/// Local time as `YYYY-MM-DD_HHMM`.
#[cfg(windows)]
pub fn local_stamp() -> String {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    // SAFETY: GetLocalTime fills the zeroed struct we pass; it cannot fail.
    let t: SYSTEMTIME = unsafe {
        let mut t = std::mem::zeroed();
        GetLocalTime(&mut t);
        t
    };
    format!(
        "{:04}-{:02}-{:02}_{:02}{:02}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute
    )
}

/// Placeholder stamp for non-Windows hosts (capture itself is Windows-only).
#[cfg(not(windows))]
pub fn local_stamp() -> String {
    "1970-01-01_0000".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_replaces_invalid_chars() {
        assert_eq!(sanitize_name("a<b>:c"), "a_b__c");
        assert_eq!(sanitize_name(r#"x/y\z|w?v*u"t"#), "x_y_z_w_v_u_t");
        assert_eq!(sanitize_name("tab\there"), "tab_here");
    }

    #[test]
    fn sanitize_reserved_names() {
        assert_eq!(sanitize_name("CON"), "_CON");
        assert_eq!(sanitize_name("nul.txt"), "_nul.txt");
        assert_eq!(sanitize_name("Com3"), "_Com3");
        assert_eq!(sanitize_name("lpt9"), "_lpt9");
        assert_eq!(sanitize_name("console"), "console");
    }

    #[test]
    fn sanitize_trims_dots_and_spaces() {
        assert_eq!(sanitize_name(" x. "), "x");
        assert_eq!(sanitize_name("..."), "recording");
    }

    #[test]
    fn sanitize_empty_becomes_recording() {
        assert_eq!(sanitize_name(""), "recording");
        assert_eq!(sanitize_name("   "), "recording");
    }

    #[test]
    fn sanitize_truncates_thai_by_chars() {
        let raw: String = "ประชุม".chars().cycle().take(100).collect();
        let out = sanitize_name(&raw);
        assert_eq!(out.chars().count(), 80);
        assert!(raw.starts_with(&out));
    }

    #[test]
    fn session_dir_creates_nested_base() {
        let base = crate::audio::temp_dir("naming-base").join("a").join("b");
        let dir = session_dir(&base, "2026-11-24_0930", "Weekly sync").unwrap();
        assert!(dir.is_dir());
        assert_eq!(dir.file_name().unwrap(), "2026-11-24_0930_Weekly sync");
    }

    #[test]
    fn session_dir_adds_suffix_on_collision() {
        let base = crate::audio::temp_dir("naming-collide");
        let first = session_dir(&base, "2026-11-24_0930", "x").unwrap();
        let second = session_dir(&base, "2026-11-24_0930", "x").unwrap();
        let third = session_dir(&base, "2026-11-24_0930", "x").unwrap();
        assert!(first.file_name().unwrap().to_str().unwrap().ends_with("_x"));
        assert!(second
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .ends_with("_x-2"));
        assert!(third
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .ends_with("_x-3"));
    }

    #[cfg(windows)]
    #[test]
    fn local_stamp_shape() {
        let s = local_stamp();
        assert_eq!(s.len(), 15);
        assert_eq!(s.as_bytes()[10], b'_');
        assert!(s[..4].parse::<u32>().unwrap() >= 2026);
    }
}
