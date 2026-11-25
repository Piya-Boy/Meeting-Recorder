//! Persistent user preferences: a tiny `key=value` file in `%APPDATA%\MeetingRecorder`.
//!
//! Only preferences are stored: how the app looks and what the next recording should use
//! (sources, devices, sample rate, transcription), never anything that was recorded.
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
    pub confirm_delete: bool,
    /// Device IDs (not list positions); empty means the Windows default.
    pub system_device: String,
    pub mic_device: String,
    /// Which sources the next recording captures.
    pub system_enabled: bool,
    pub mic_enabled: bool,
    /// Position in the sample-rate list of Settings > Audio.
    pub sample_rate_idx: usize,
    /// Show transcript text while recording, and how often it is refreshed (list position).
    pub partial_transcription: bool,
    pub chunk_window_idx: usize,
    /// Model weights: `int8` or `fp32`.
    pub asr_precision: String,
    /// ONNX Runtime threads for transcription: 4, 6 or 8.
    pub asr_threads: u32,
    /// How the model writes: `th` (Thai letters) or `th-en` (English words in Latin letters).
    pub asr_language: String,
    /// Word fixes, one rule a line (see `asr::glossary`).
    pub asr_glossary: Vec<String>,
    /// Ask GitHub for a newer version at start-up.
    pub update_check: bool,
    /// Seconds since 1970 of the last answer from GitHub (0 = never).
    pub update_last_check: u64,
}

impl Default for Persisted {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::Dark,
            accent_idx: 0,
            font_scale_idx: 1,
            language_idx: 0,
            confirm_delete: true,
            system_device: String::new(),
            mic_device: String::new(),
            system_enabled: true,
            mic_enabled: true,
            sample_rate_idx: 0,
            partial_transcription: true,
            chunk_window_idx: 1,
            asr_precision: "int8".to_owned(),
            asr_threads: 4,
            asr_language: "th".to_owned(),
            asr_glossary: Vec::new(),
            update_check: true,
            update_last_check: 0,
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
        let mut text = format!(
            "theme={}\naccent={}\nfont_scale={}\nlanguage={}\nconfirm_delete={}\naudio.system_device={}\naudio.mic_device={}\nasr.precision={}\nasr.threads={}\n",
            theme_name(self.theme_mode),
            self.accent_idx,
            self.font_scale_idx,
            self.language_idx,
            self.confirm_delete,
            self.system_device,
            self.mic_device,
            self.asr_precision,
            self.asr_threads,
        );
        text.push_str(&format!(
            "audio.system_enabled={}\naudio.mic_enabled={}\naudio.sample_rate_index={}\nasr.partial={}\nasr.chunk_window_index={}\n",
            self.system_enabled,
            self.mic_enabled,
            self.sample_rate_idx,
            self.partial_transcription,
            self.chunk_window_idx,
        ));
        text.push_str(&format!("asr.language={}\n", self.asr_language));
        for rule in &self.asr_glossary {
            text.push_str(&format!("asr.glossary={rule}\n"));
        }
        text.push_str(&format!(
            "update.check={}\nupdate.last_check={}\n",
            self.update_check, self.update_last_check
        ));
        text
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
                "confirm_delete" => set_bool(&mut out.confirm_delete, value),
                "audio.system_device" => out.system_device = value.to_owned(),
                "audio.mic_device" => out.mic_device = value.to_owned(),
                "audio.system_enabled" => set_bool(&mut out.system_enabled, value),
                "audio.mic_enabled" => set_bool(&mut out.mic_enabled, value),
                "audio.sample_rate_index" => set_index(&mut out.sample_rate_idx, value, 3),
                "asr.partial" => set_bool(&mut out.partial_transcription, value),
                "asr.chunk_window_index" => set_index(&mut out.chunk_window_idx, value, 3),
                "asr.precision" => {
                    if matches!(value, "int8" | "fp32") {
                        out.asr_precision = value.to_owned();
                    }
                }
                "asr.language" => {
                    if matches!(value, "th" | "th-en") {
                        out.asr_language = value.to_owned();
                    }
                }
                "update.check" => set_bool(&mut out.update_check, value),
                "update.last_check" => {
                    if let Ok(n) = value.parse::<u64>() {
                        out.update_last_check = n;
                    }
                }
                "asr.glossary" => {
                    if !value.is_empty() {
                        out.asr_glossary.push(value.to_owned());
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
            confirm_delete: false,
            system_device: String::new(),
            mic_device: String::new(),
            system_enabled: false,
            mic_enabled: false,
            sample_rate_idx: 2,
            partial_transcription: false,
            chunk_window_idx: 2,
            asr_precision: "int8".to_owned(),
            asr_threads: 6,
            asr_language: "th".to_owned(),
            asr_glossary: Vec::new(),
            update_check: true,
            update_last_check: 0,
        };
        assert_eq!(Persisted::from_text(&s.to_text()), s);
    }

    #[test]
    fn a_file_without_the_recorder_keys_keeps_the_defaults() {
        // Every file saved before these keys existed.
        let s = Persisted::from_text("theme=light\naudio.mic_device=m\n");
        assert!(s.system_enabled && s.mic_enabled && s.partial_transcription);
        assert_eq!((s.sample_rate_idx, s.chunk_window_idx), (0, 1));
    }

    #[test]
    fn bad_recorder_values_keep_the_defaults() {
        let s = Persisted::from_text(
            "audio.system_enabled=no\naudio.mic_enabled=0\naudio.sample_rate_index=3\n\
             asr.partial=off\nasr.chunk_window_index=-1\n",
        );
        assert_eq!(s, Persisted::default());
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
    fn keys_of_removed_settings_are_ignored_and_not_written_again() {
        // v0.0.1 wrote these two; `minimize_to_tray=true` is in every file it saved.
        let s = Persisted::from_text(
            "theme=light\nstart_with_windows=true\nminimize_to_tray=true\nconfirm_delete=false\n",
        );
        assert_eq!(s.theme_mode, ThemeMode::Light);
        assert!(!s.confirm_delete);
        let text = s.to_text();
        assert!(!text.contains("start_with_windows"), "{text}");
        assert!(!text.contains("minimize_to_tray"), "{text}");
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

    #[test]
    fn language_and_word_fixes_round_trip() {
        let s = Persisted {
            asr_language: "th-en".to_owned(),
            asr_glossary: vec!["มอล, หมอก = มอก.".to_owned(), "a = b = c".to_owned()],
            ..Persisted::default()
        };
        let text = s.to_text();
        assert!(text.contains("asr.language=th-en\n"), "{text}");
        assert!(text.contains("asr.glossary=มอล, หมอก = มอก.\n"), "{text}");
        assert_eq!(Persisted::from_text(&text), s);
    }

    #[test]
    fn missing_or_bad_language_is_thai_and_no_word_fixes() {
        let s = Persisted::from_text("theme=light\nasr.language=xx\n");
        assert_eq!(s.asr_language, "th");
        assert!(s.asr_glossary.is_empty());
    }

    #[test]
    fn update_settings_round_trip() {
        let s = Persisted {
            update_check: false,
            update_last_check: 1_764_000_000,
            ..Persisted::default()
        };
        let text = s.to_text();
        assert!(text.contains("update.check=false\n"), "{text}");
        assert!(text.contains("update.last_check=1764000000\n"), "{text}");
        assert_eq!(Persisted::from_text(&text), s);
    }

    #[test]
    fn missing_or_bad_update_settings_keep_the_defaults() {
        for text in [
            "theme=light\n",
            "update.check=maybe\nupdate.last_check=abc\n",
        ] {
            let s = Persisted::from_text(text);
            assert!(s.update_check, "{text:?}");
            assert_eq!(s.update_last_check, 0, "{text:?}");
        }
    }
}
