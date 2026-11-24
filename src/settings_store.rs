//! Persistent user preferences: a tiny `key=value` file in `%APPDATA%\MeetingRecorder`.
//!
//! Only preferences are stored (theme, accent, font scale, language, general toggles).
//! Unknown keys and malformed lines are ignored, so older and newer builds can share a file.

use std::path::PathBuf;

use crate::theme::ThemeMode;

/// Settings that survive an app restart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Persisted {
    pub theme_mode: ThemeMode,
    pub accent_idx: usize,
    pub font_scale_idx: usize,
    pub language_idx: usize,
    pub start_with_windows: bool,
    pub minimize_to_tray: bool,
    pub confirm_delete: bool,
    /// Device IDs (not list positions); empty means the Windows default.
    pub system_device: String,
    pub mic_device: String,
    /// Model weights: `int8` or `fp32`.
    pub asr_precision: String,
    /// ONNX Runtime threads for transcription: 4, 6 or 8.
    pub asr_threads: u32,
}

impl Default for Persisted {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::Dark,
            accent_idx: 0,
            font_scale_idx: 1,
            language_idx: 0,
            start_with_windows: false,
            minimize_to_tray: true,
            confirm_delete: true,
            system_device: String::new(),
            mic_device: String::new(),
            asr_precision: "int8".to_owned(),
            asr_threads: 4,
        }
    }
}

fn theme_name(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::Dark => "dark",
        ThemeMode::Light => "light",
        ThemeMode::System => "system",
    }
}

impl Persisted {
    pub fn to_text(&self) -> String {
        format!(
            "theme={}\naccent={}\nfont_scale={}\nlanguage={}\nstart_with_windows={}\nminimize_to_tray={}\nconfirm_delete={}\naudio.system_device={}\naudio.mic_device={}\nasr.precision={}\nasr.threads={}\n",
            theme_name(self.theme_mode),
            self.accent_idx,
            self.font_scale_idx,
            self.language_idx,
            self.start_with_windows,
            self.minimize_to_tray,
            self.confirm_delete,
            self.system_device,
            self.mic_device,
            self.asr_precision,
            self.asr_threads,
        )
    }

    /// Parses stored text on top of the defaults; bad values keep the default.
    pub fn from_text(text: &str) -> Self {
        let mut out = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            match key {
                "theme" => match value {
                    "dark" => out.theme_mode = ThemeMode::Dark,
                    "light" => out.theme_mode = ThemeMode::Light,
                    "system" => out.theme_mode = ThemeMode::System,
                    _ => {}
                },
                "accent" => set_index(&mut out.accent_idx, value, 3),
                "font_scale" => set_index(&mut out.font_scale_idx, value, 4),
                "language" => set_index(&mut out.language_idx, value, 2),
                "start_with_windows" => set_bool(&mut out.start_with_windows, value),
                "minimize_to_tray" => set_bool(&mut out.minimize_to_tray, value),
                "confirm_delete" => set_bool(&mut out.confirm_delete, value),
                "audio.system_device" => out.system_device = value.to_owned(),
                "audio.mic_device" => out.mic_device = value.to_owned(),
                "asr.precision" => {
                    if matches!(value, "int8" | "fp32") {
                        out.asr_precision = value.to_owned();
                    }
                }
                "asr.threads" => {
                    if let Some(n) = value.parse::<u32>().ok().filter(|n| matches!(n, 4 | 6 | 8)) {
                        out.asr_threads = n;
                    }
                }
                _ => {}
            }
        }
        out
    }
}

/// Accepts `value` only if it is an index below `len`.
fn set_index(target: &mut usize, value: &str, len: usize) {
    if let Ok(v) = value.parse::<usize>() {
        if v < len {
            *target = v;
        }
    }
}

fn set_bool(target: &mut bool, value: &str) {
    match value {
        "true" => *target = true,
        "false" => *target = false,
        _ => {}
    }
}

fn config_path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA").or_else(|| std::env::var_os("XDG_CONFIG_HOME"))?;
    Some(
        PathBuf::from(base)
            .join("MeetingRecorder")
            .join("settings.ini"),
    )
}

/// Loads saved settings; missing or unreadable file gives the defaults.
pub fn load() -> Persisted {
    config_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|t| Persisted::from_text(&t))
        .unwrap_or_default()
}

/// Saves settings atomically (temp file, then rename). Failures are ignored: losing a
/// preference must never interrupt the app.
pub fn save(settings: &Persisted) {
    let Some(path) = config_path() else {
        return;
    };
    let Some(dir) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let tmp = path.with_extension("ini.tmp");
    if std::fs::write(&tmp, settings.to_text()).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let s = Persisted {
            theme_mode: ThemeMode::Light,
            accent_idx: 2,
            font_scale_idx: 3,
            language_idx: 1,
            start_with_windows: true,
            minimize_to_tray: false,
            confirm_delete: false,
            system_device: String::new(),
            mic_device: String::new(),
            asr_precision: "int8".to_owned(),
            asr_threads: 6,
        };
        assert_eq!(Persisted::from_text(&s.to_text()), s);
    }

    #[test]
    fn device_ids_round_trip() {
        let s = Persisted {
            system_device: "{0.0.0.00000000}.{abc-123}".to_owned(),
            mic_device: "{0.0.1.00000000}.{def-456}".to_owned(),
            ..Persisted::default()
        };
        let text = s.to_text();
        assert!(text.contains(
            "audio.system_device={0.0.0.00000000}.{abc-123}
"
        ));
        assert_eq!(Persisted::from_text(&text), s);
    }

    #[test]
    fn missing_device_keys_default_to_empty() {
        let s = Persisted::from_text(
            "theme=light
",
        );
        assert_eq!(s.system_device, "");
        assert_eq!(s.mic_device, "");
    }

    #[test]
    fn transcription_settings_round_trip() {
        let s = Persisted {
            asr_precision: "fp32".to_owned(),
            asr_threads: 8,
            ..Persisted::default()
        };
        let text = s.to_text();
        assert!(text.contains("asr.precision=fp32\n"), "{text}");
        assert!(text.contains("asr.threads=8\n"), "{text}");
        assert_eq!(Persisted::from_text(&text), s);
    }

    #[test]
    fn missing_transcription_keys_default_to_int8_and_4_threads() {
        let s = Persisted::from_text("theme=light\n");
        assert_eq!(s.asr_precision, "int8");
        assert_eq!(s.asr_threads, 4);
    }

    #[test]
    fn unknown_transcription_values_fall_back() {
        let s = Persisted::from_text("asr.threads=7\nasr.precision=weird\n");
        assert_eq!(s.asr_threads, 4);
        assert_eq!(s.asr_precision, "int8");
        let s = Persisted::from_text("asr.threads=0\n");
        assert_eq!(s.asr_threads, 4);
        let s = Persisted::from_text("asr.threads=six\n");
        assert_eq!(s.asr_threads, 4);
    }

    #[test]
    fn bad_input_keeps_defaults() {
        let s = Persisted::from_text(
            "theme=purple\naccent=99\nlanguage=-1\nnoise\n=\nconfirm_delete=maybe\nfuture_key=1",
        );
        assert_eq!(s, Persisted::default());
    }
}
