use crate::i18n::tr;
use egui::Ui;

use crate::screens::{
    recorder::{draw_recorder_screen, RecorderScreen},
    recordings::{draw_recordings_screen, RecordingsScreen},
    settings::{
        advanced::{draw_settings_advanced, SettingsAdvancedScreen},
        appearance::{draw_settings_appearance, SettingsAppearanceScreen},
        audio::{draw_settings_audio, SettingsAudioScreen},
        category_rail as draw_settings_category_rail,
        general::{draw_settings_general, SettingsGeneralScreen},
        storage::{draw_settings_storage, SettingsStorageScreen},
        transcription::{draw_settings_transcription, SettingsTranscriptionScreen},
    },
    transcript_detail::{draw_transcript_detail_screen, TranscriptDetailScreen},
    transcripts::{draw_transcripts_screen, TranscriptsScreen},
};
use crate::settings_store::{self, Persisted};
use crate::shell;
use crate::theme::{apply_theme, ThemeColors, ThemeMode};

// ─── Screen routing ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Screen {
    #[default]
    Recorder,
    Recordings,
    Transcripts,
    TranscriptDetail,
    SettingsGeneral,
    SettingsAudio,
    SettingsTranscription,
    SettingsStorage,
    SettingsAppearance,
    SettingsAdvanced,
}

impl Screen {
    pub fn title(self) -> &'static str {
        match self {
            Screen::Recorder => tr("Recorder"),
            Screen::Recordings => tr("Recordings"),
            Screen::Transcripts => tr("Transcripts"),
            Screen::TranscriptDetail => tr("Transcript Detail"),
            Screen::SettingsGeneral => tr("Settings"),
            Screen::SettingsAudio => tr("Settings"),
            Screen::SettingsTranscription => tr("Settings"),
            Screen::SettingsStorage => tr("Settings"),
            Screen::SettingsAppearance => tr("Settings"),
            Screen::SettingsAdvanced => tr("Settings"),
        }
    }

    pub fn subtitle(self) -> Option<&'static str> {
        match self {
            Screen::SettingsGeneral => Some(tr("General & Shortcuts")),
            Screen::SettingsAudio => Some(tr("Audio (Windows WASAPI)")),
            Screen::SettingsTranscription => Some(tr("Transcription (OpenThai ASR)")),
            Screen::SettingsStorage => Some(tr("Storage & SQLite")),
            Screen::SettingsAppearance => Some(tr("Appearance & Theme")),
            Screen::SettingsAdvanced => Some(tr("Advanced & About")),
            _ => None,
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "recorder" => Screen::Recorder,
            "recordings" => Screen::Recordings,
            "transcripts" => Screen::Transcripts,
            "transcript-detail" => Screen::TranscriptDetail,
            "settings-general" => Screen::SettingsGeneral,
            "settings-audio" => Screen::SettingsAudio,
            "settings-transcription" => Screen::SettingsTranscription,
            "settings-storage" => Screen::SettingsStorage,
            "settings-appearance" => Screen::SettingsAppearance,
            "settings-advanced" => Screen::SettingsAdvanced,
            _ => return None,
        })
    }

    pub fn is_settings(self) -> bool {
        matches!(
            self,
            Screen::SettingsGeneral
                | Screen::SettingsAudio
                | Screen::SettingsTranscription
                | Screen::SettingsStorage
                | Screen::SettingsAppearance
                | Screen::SettingsAdvanced
        )
    }
}

// ─── Modal overlay ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Modal {
    Help,
    About,
    DeleteRecording { id: usize, name: String },
}

// ─── Application state ───────────────────────────────────────────────────────

pub struct AppState {
    pub theme_mode: ThemeMode,
    pub current_screen: Screen,
    pub sidebar_collapsed: bool,
    pub open_modal: Option<Modal>,

    // Screen states
    pub recorder: RecorderScreen,
    pub recordings: RecordingsScreen,
    pub transcripts: TranscriptsScreen,
    pub transcript_detail: TranscriptDetailScreen,
    pub settings_general: SettingsGeneralScreen,
    pub settings_audio: SettingsAudioScreen,
    pub settings_transcription: SettingsTranscriptionScreen,
    pub settings_storage: SettingsStorageScreen,
    pub settings_appearance: SettingsAppearanceScreen,
    pub settings_advanced: SettingsAdvancedScreen,

    /// Whether preference changes are written to disk (off for dev-flag runs and tests).
    persist: bool,
    saved: Persisted,
    /// True after Tab until the next mouse press: only then is the focus ring drawn.
    keyboard_nav: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::Dark,
            current_screen: Screen::Recorder,
            sidebar_collapsed: false,
            open_modal: None,
            recorder: RecorderScreen::default(),
            recordings: RecordingsScreen::default(),
            transcripts: TranscriptsScreen::default(),
            transcript_detail: TranscriptDetailScreen::default(),
            settings_general: SettingsGeneralScreen::default(),
            settings_audio: SettingsAudioScreen::default(),
            settings_transcription: SettingsTranscriptionScreen::default(),
            settings_storage: SettingsStorageScreen::default(),
            settings_appearance: SettingsAppearanceScreen::default(),
            settings_advanced: SettingsAdvancedScreen::default(),
            persist: false,
            saved: Persisted::default(),
            keyboard_nav: false,
        }
    }
}

impl AppState {
    /// Dev/preview flags: `--screen=<name>`, `--theme=light|dark`,
    /// `--state=<recorder state>`, `--modal=help|about|delete`, `--lang=en|th`, `--engine=loading|ready|error`.
    /// Unknown values are ignored.
    pub fn from_args(args: impl Iterator<Item = String>) -> Self {
        let mut state = Self::default();
        for arg in args {
            if let Some(name) = arg.strip_prefix("--screen=") {
                if let Some(screen) = Screen::from_name(name) {
                    state.current_screen = screen;
                }
            } else if let Some(name) = arg.strip_prefix("--state=") {
                use crate::screens::recorder::RecorderState;
                state.recorder.state = match name {
                    "recording" => RecorderState::Recording,
                    "paused" => RecorderState::Paused,
                    "processing" => RecorderState::Processing,
                    "error" => RecorderState::Error,
                    _ => RecorderState::Idle,
                };
            } else if let Some(name) = arg.strip_prefix("--modal=") {
                state.open_modal = match name {
                    "help" => Some(Modal::Help),
                    "about" => Some(Modal::About),
                    "delete" => Some(Modal::DeleteRecording {
                        id: 1,
                        name: "Q4 Engineering Architecture Sync".to_owned(),
                    }),
                    _ => None,
                };
            } else if let Some(mode) = arg.strip_prefix("--theme=") {
                match mode {
                    "light" => state.theme_mode = ThemeMode::Light,
                    "dark" => state.theme_mode = ThemeMode::Dark,
                    _ => {}
                }
            } else if let Some(engine) = arg.strip_prefix("--engine=") {
                use crate::screens::recorder::EngineState;
                let forced = match engine {
                    "loading" => Some(EngineState::Loading),
                    "ready" => Some(EngineState::Ready),
                    "error" => Some(EngineState::Error),
                    _ => None,
                };
                if let Some(e) = forced {
                    state.recorder.engine = e;
                    state.recorder.engine_auto = false;
                }
            } else if let Some(lang) = arg.strip_prefix("--lang=") {
                match lang {
                    "en" => state.settings_general.language_idx = 0,
                    "th" => state.settings_general.language_idx = 1,
                    _ => {}
                }
            }
        }
        state
    }

    /// Applies saved preferences and starts writing future changes back to disk.
    pub fn enable_persistence(&mut self, saved: Persisted) {
        self.theme_mode = saved.theme_mode;
        self.settings_appearance.accent_idx = saved.accent_idx;
        self.settings_appearance.font_scale_idx = saved.font_scale_idx;
        self.settings_general.language_idx = saved.language_idx;
        self.settings_general.start_with_windows = saved.start_with_windows;
        self.settings_general.minimize_to_tray = saved.minimize_to_tray;
        self.settings_general.confirm_delete = saved.confirm_delete;
        self.saved = self.snapshot();
        self.persist = true;
    }

    fn snapshot(&self) -> Persisted {
        Persisted {
            theme_mode: self.theme_mode,
            accent_idx: self.settings_appearance.accent_idx,
            font_scale_idx: self.settings_appearance.font_scale_idx,
            language_idx: self.settings_general.language_idx,
            start_with_windows: self.settings_general.start_with_windows,
            minimize_to_tray: self.settings_general.minimize_to_tray,
            confirm_delete: self.settings_general.confirm_delete,
        }
    }

    fn save_if_changed(&mut self) {
        if !self.persist {
            return;
        }
        let now = self.snapshot();
        if now != self.saved {
            settings_store::save(&now);
            self.saved = now;
        }
    }

    /// App-wide shortcuts listed in Settings > General (Ctrl+Shift+R / P / A / C).
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        use egui::{Key, Modifiers};
        let mods = Modifiers::CTRL | Modifiers::SHIFT;
        let (record, pause, auto_scroll, copy) = ctx.input_mut(|i| {
            (
                i.consume_key(mods, Key::R),
                i.consume_key(mods, Key::P),
                i.consume_key(mods, Key::A),
                i.consume_key(mods, Key::C),
            )
        });
        if record {
            self.recorder.toggle_recording();
        }
        if pause {
            self.recorder.toggle_pause();
        }
        if auto_scroll {
            self.recorder.auto_scroll = !self.recorder.auto_scroll;
        }
        if copy {
            ctx.copy_text(self.recorder.transcript_text());
            crate::toast::show(ctx, "Transcript copied to clipboard");
        }
    }

    /// Outlines the keyboard-focused widget (custom-painted widgets draw no focus state).
    fn draw_focus_ring(&mut self, ctx: &egui::Context, theme: &ThemeColors) {
        if ctx.input(|i| i.key_pressed(egui::Key::Tab)) {
            self.keyboard_nav = true;
        }
        if ctx.input(|i| i.pointer.any_pressed()) {
            self.keyboard_nav = false;
        }
        if !self.keyboard_nav {
            return;
        }
        let Some(id) = ctx.memory(|m| m.focused()) else {
            return;
        };
        let Some(resp) = ctx.read_response(id) else {
            return;
        };
        ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("focus_ring"),
        ))
        .rect_stroke(
            resp.rect.expand(2.0),
            egui::CornerRadius::same(8),
            egui::Stroke::new(2.0, theme.brand_blue),
            egui::StrokeKind::Outside,
        );
    }

    fn theme(&self) -> ThemeColors {
        ThemeColors::get(self.theme_mode).with_accent(self.settings_appearance.accent_idx)
    }
}

impl eframe::App for AppState {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        crate::i18n::set_lang(crate::i18n::lang_from_index(
            self.settings_general.language_idx,
        ));
        let now = ctx.input(|i| i.time);
        if self.recorder.tick_engine(now) {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        self.settings_transcription.engine = self.recorder.engine;
        if self.open_modal.is_none() {
            self.handle_shortcuts(ctx);
        }
        let theme = self.theme();
        apply_theme(ctx, &theme);
        let zoom = self.settings_appearance.zoom_factor();
        if (ctx.zoom_factor() - zoom).abs() > f32::EPSILON {
            ctx.set_zoom_factor(zoom);
        }

        // ── Sidebar ──────────────────────────────────────────────────────────
        let sidebar_w = if self.sidebar_collapsed {
            64.0_f32
        } else {
            220.0_f32
        };

        egui::SidePanel::left("main_sidebar")
            .exact_width(sidebar_w)
            .resizable(false)
            .frame(egui::Frame::NONE.fill(theme.sidebar))
            .show(ctx, |ui| {
                shell::draw_sidebar(ui, self, &theme);
            });

        // ── Top bar ──────────────────────────────────────────────────────────
        egui::TopBottomPanel::top("main_topbar")
            .exact_height(48.0)
            .resizable(false)
            .frame(
                egui::Frame::NONE
                    .fill(theme.surface)
                    .inner_margin(egui::Margin::symmetric(24, 0)),
            )
            .show(ctx, |ui| {
                let full = ui.max_rect().expand2(egui::vec2(24.0, 0.0));
                shell::window_drag_region(ui, full, "topbar_drag");
                let mut content = ui.max_rect();
                content.max.x -= shell::WINDOW_CONTROLS_WIDTH - 24.0;
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(content)
                        .layout(*ui.layout()),
                    |ui| shell::draw_topbar(ui, self, &theme),
                );
            });

        // ── Central panel ────────────────────────────────────────────────────
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(theme.background))
            .show(ctx, |ui| {
                draw_content(ui, self, &theme);
            });

        // "Confirm before deleting recordings" off: delete immediately.
        if !self.settings_general.confirm_delete {
            if let Some(Modal::DeleteRecording { id, .. }) = self.open_modal.clone() {
                self.recordings.recordings.retain(|r| r.id != id);
                if self.recordings.playback_id == Some(id) {
                    self.recordings.playback_id = None;
                }
                self.open_modal = None;
            }
        }

        shell::draw_window_controls(ctx, &theme);
        shell::handle_window_resize(ctx);
        crate::toast::draw(ctx, &theme);

        // ── Modal overlay ────────────────────────────────────────────────────
        crate::modals::draw(ctx, self, &theme);

        self.draw_focus_ring(ctx, &theme);
        self.save_if_changed();
    }
}

// ─── Main content area ───────────────────────────────────────────────────────

fn draw_content(ui: &mut Ui, state: &mut AppState, theme: &ThemeColors) {
    match state.current_screen {
        Screen::Recorder => {
            draw_recorder_screen(ui, theme, &mut state.recorder);
        }
        Screen::Recordings => {
            if let Some(target) =
                draw_recordings_screen(ui, theme, &mut state.recordings, &mut state.open_modal)
            {
                state.current_screen = target;
            }
        }
        Screen::Transcripts => {
            if let Some(target) = draw_transcripts_screen(ui, theme, &mut state.transcripts) {
                state.current_screen = target;
            }
        }
        Screen::TranscriptDetail => {
            draw_transcript_detail_screen(ui, theme, &mut state.transcript_detail);
        }
        Screen::SettingsGeneral
        | Screen::SettingsAudio
        | Screen::SettingsTranscription
        | Screen::SettingsStorage
        | Screen::SettingsAppearance
        | Screen::SettingsAdvanced => {
            draw_settings_content(ui, state, theme);
        }
    }
}

fn draw_settings_content(ui: &mut Ui, state: &mut AppState, theme: &ThemeColors) {
    if let Some(picked) = draw_settings_category_rail(ui, state.current_screen, theme) {
        state.current_screen = picked;
    }

    match state.current_screen {
        Screen::SettingsGeneral => draw_settings_general(ui, &mut state.settings_general, theme),
        Screen::SettingsAudio => draw_settings_audio(ui, &mut state.settings_audio, theme),
        Screen::SettingsTranscription => {
            draw_settings_transcription(ui, &mut state.settings_transcription, theme)
        }
        Screen::SettingsStorage => draw_settings_storage(ui, &mut state.settings_storage, theme),
        Screen::SettingsAppearance => draw_settings_appearance(
            ui,
            &mut state.settings_appearance,
            theme,
            &mut state.theme_mode,
        ),
        Screen::SettingsAdvanced => draw_settings_advanced(ui, &mut state.settings_advanced, theme),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> impl Iterator<Item = String> {
        list.iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn every_screen_name_round_trips() {
        for name in [
            "recorder",
            "recordings",
            "transcripts",
            "transcript-detail",
            "settings-general",
            "settings-audio",
            "settings-transcription",
            "settings-storage",
            "settings-appearance",
            "settings-advanced",
        ] {
            assert!(Screen::from_name(name).is_some(), "{name}");
        }
        assert!(Screen::from_name("nope").is_none());
    }

    #[test]
    fn dev_flags_select_screen_theme_and_state() {
        let s = AppState::from_args(args(&[
            "--screen=settings-audio",
            "--theme=light",
            "--state=paused",
        ]));
        assert_eq!(s.current_screen, Screen::SettingsAudio);
        assert_eq!(s.theme_mode, ThemeMode::Light);
        assert_eq!(
            s.recorder.state,
            crate::screens::recorder::RecorderState::Paused
        );
    }

    #[test]
    fn unknown_flags_are_ignored() {
        let s = AppState::from_args(args(&["--screen=bogus", "--theme=neon", "--wat"]));
        assert_eq!(s.current_screen, Screen::Recorder);
        assert_eq!(s.theme_mode, ThemeMode::Dark);
    }

    #[test]
    fn only_settings_screens_report_is_settings() {
        assert!(Screen::SettingsStorage.is_settings());
        assert!(!Screen::Transcripts.is_settings());
        assert!(!Screen::TranscriptDetail.is_settings());
    }
}
