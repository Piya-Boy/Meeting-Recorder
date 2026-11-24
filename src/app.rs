use crate::asr::model::{ModelStatus, Precision};
use crate::asr::worker::{AsrHandle, Command as AsrCommand, EngineEvent, Event as AsrEvent};
use crate::audio::session::{Command, Event};
use crate::audio::worker::AudioHandle;
use crate::i18n::tr;
use egui::Ui;

use crate::screens::{
    recorder::{draw_recorder_screen, AsrJob, AsrRequest, EngineState, RecorderScreen},
    recordings::{draw_recordings_screen, RecordingsRequest, RecordingsScreen},
    settings::{
        advanced::{draw_settings_advanced, SettingsAdvancedScreen},
        appearance::{draw_settings_appearance, SettingsAppearanceScreen},
        audio::{draw_settings_audio, AudioChoice, SettingsAudioScreen},
        category_rail as draw_settings_category_rail,
        general::{draw_settings_general, SettingsGeneralScreen},
        storage::{draw_settings_storage, SettingsStorageScreen},
        transcription::{
            draw_settings_transcription, threads_index, ModelAction, SettingsTranscriptionScreen,
        },
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
    /// `dir` is the session folder; `None` for the preview's sample rows.
    DeleteRecording {
        id: usize,
        name: String,
        dir: Option<std::path::PathBuf>,
    },
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
    /// Devices Windows reports and the ones picked (Recorder and Settings > Audio share it).
    pub audio_choice: AudioChoice,
    /// The transcription worker; `None` in preview mode (dev flags, tests).
    asr: Option<AsrHandle>,
    /// Precision the worker was last asked about.
    asr_precision_sent: Precision,
    /// One-shot text for a toast about the model (taken once per frame).
    asr_notice: Option<String>,
    /// Lists the recordings on disk for the Recordings and Transcripts screens.
    library: crate::library::LibraryHandle,
    /// True once the screens show real folders (the capture worker is connected).
    library_live: bool,
    /// The screen of the last frame, to refresh the lists when one is opened.
    last_screen: Screen,
    /// A delete was sent and its answer has not come yet.
    asr_deleting: bool,

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
            audio_choice: AudioChoice::default(),
            asr: None,
            asr_precision_sent: Precision::Int8,
            asr_notice: None,
            library: crate::library::LibraryHandle::new(),
            library_live: false,
            last_screen: Screen::default(),
            asr_deleting: false,
            persist: false,
            saved: Persisted::default(),
            keyboard_nav: false,
        }
    }
}

impl AppState {
    /// Dev/preview flags: `--screen=<name>`, `--theme=light|dark`,
    /// `--state=<recorder state>`, `--modal=help|about|delete`, `--lang=en|th`, `--engine=loading|ready|error|missing|downloading`.
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
                    "starting" => RecorderState::Starting,
                    "recording" => RecorderState::Recording,
                    "paused" => RecorderState::Paused,
                    "processing" => RecorderState::Processing,
                    "saved" => RecorderState::Saved,
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
                        dir: None,
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
                let forced = match engine {
                    "loading" => Some(EngineState::Loading),
                    "ready" => Some(EngineState::Ready),
                    "error" => Some(EngineState::Error),
                    "missing" => Some(EngineState::Missing),
                    "downloading" => Some(EngineState::Downloading),
                    _ => None,
                };
                if let Some(e) = forced {
                    state.recorder.engine = e;
                    state.recorder.engine_auto = false;
                    state.settings_transcription.model_status = match e {
                        EngineState::Ready => ModelStatus::Ready,
                        EngineState::Downloading => ModelStatus::Downloading {
                            done: 412_000_000,
                            total: 883_000_000,
                        },
                        EngineState::Error => ModelStatus::Failed("connection lost".to_owned()),
                        EngineState::Missing | EngineState::Loading => ModelStatus::Missing,
                    };
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
        self.audio_choice.system_id = saved.system_device;
        self.audio_choice.mic_id = saved.mic_device;
        self.settings_transcription.precision = Precision::from_str_lossy(&saved.asr_precision);
        self.settings_transcription.threads_idx = threads_index(saved.asr_threads);
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
            system_device: self.audio_choice.system_id.clone(),
            mic_device: self.audio_choice.mic_id.clone(),
            asr_precision: self.settings_transcription.precision.as_str().to_owned(),
            asr_threads: self.settings_transcription.threads() as u32,
        }
    }

    /// Connects the transcription worker and asks whether the model is installed.
    pub fn enable_asr(&mut self, handle: AsrHandle) {
        let precision = self.settings_transcription.precision;
        self.recorder.engine_auto = false;
        self.recorder.engine = EngineState::Missing;
        self.asr_precision_sent = precision;
        handle.send(AsrCommand::CheckModel { precision });
        self.asr = Some(handle);
    }

    /// Applies one report from the transcription worker.
    pub fn apply_asr_event(&mut self, event: AsrEvent) {
        match event {
            AsrEvent::Model(status) => {
                let was_downloading = matches!(
                    self.settings_transcription.model_status,
                    ModelStatus::Downloading { .. } | ModelStatus::Verifying
                );
                self.recorder.engine = match &status {
                    ModelStatus::Ready => EngineState::Ready,
                    ModelStatus::Missing => EngineState::Missing,
                    ModelStatus::Downloading { .. } => EngineState::Downloading,
                    ModelStatus::Verifying => EngineState::Loading,
                    ModelStatus::Failed(_) => EngineState::Error,
                };
                match &status {
                    ModelStatus::Ready if was_downloading => {
                        self.asr_notice = Some("Model downloaded".to_owned());
                    }
                    ModelStatus::Failed(_) if was_downloading => {
                        self.asr_notice = Some("Download failed".to_owned());
                    }
                    ModelStatus::Missing if self.asr_deleting => {
                        self.asr_notice = Some("Model deleted".to_owned());
                    }
                    _ => {}
                }
                if !matches!(
                    status,
                    ModelStatus::Downloading { .. } | ModelStatus::Verifying
                ) {
                    self.asr_deleting = false;
                }
                self.settings_transcription.model_status = status;
            }
            AsrEvent::Engine(EngineEvent::Loading) => self.recorder.engine = EngineState::Loading,
            AsrEvent::Engine(EngineEvent::Ready) => self.recorder.engine = EngineState::Ready,
            AsrEvent::Engine(EngineEvent::Failed(_)) => self.recorder.engine = EngineState::Error,
            AsrEvent::Progress {
                done_secs,
                total_secs,
                dir,
            } => self.recorder.asr_progress(&dir, done_secs, total_secs),
            AsrEvent::Done { dir, transcript } => {
                self.recorder.asr_done(&dir, &transcript);
                self.refresh_library();
            }
            AsrEvent::Cancelled { dir } => {
                self.recorder.asr_cancelled(&dir);
                self.asr_notice = Some("Transcription cancelled".to_owned());
                self.refresh_library();
            }
            AsrEvent::Error {
                dir: Some(dir),
                message,
            } => {
                self.recorder.asr_failed(&dir, message);
                self.refresh_library();
            }
            AsrEvent::Error { dir: None, message } => self.asr_notice = Some(message),
            // The job waits behind another; the panel keeps showing the running one.
            AsrEvent::Queued { .. } => {}
        }
    }

    /// Sends a saved recording to the worker, or notes that it has to wait for the model.
    /// Does nothing in preview mode (no worker).
    fn start_transcription(&mut self, dir: std::path::PathBuf) {
        if self.asr.is_none() {
            return;
        }
        if self.settings_transcription.model_status != ModelStatus::Ready {
            self.recorder.asr_waiting(dir);
            self.asr_notice = Some(
                "Waiting for the model. The recording is saved; transcribe it after the model is downloaded."
                    .to_owned(),
            );
            return;
        }
        if let Some(asr) = &self.asr {
            asr.send(AsrCommand::Transcribe {
                dir: dir.clone(),
                precision: self.settings_transcription.precision,
                threads: self.settings_transcription.threads(),
            });
        }
        self.recorder.asr_started(dir);
    }

    fn handle_recorder_request(&mut self, request: AsrRequest) {
        match request {
            AsrRequest::OpenSettings => self.current_screen = Screen::SettingsTranscription,
            AsrRequest::Transcribe => {
                if let Some(dir) = self.recorder.saved_dir.clone() {
                    self.start_transcription(dir);
                }
            }
            AsrRequest::Cancel => {
                if let Some(asr) = &self.asr {
                    asr.send(AsrCommand::CancelTranscribe);
                }
            }
        }
    }

    /// Text for a toast about the model, once.
    pub fn take_asr_notice(&mut self) -> Option<String> {
        self.asr_notice.take()
    }

    /// Passes worker reports to the screens and the screens' requests to the worker.
    pub fn sync_asr(&mut self) {
        if let Some(request) = self.recorder.take_request() {
            self.handle_recorder_request(request);
        }
        let events = match &self.asr {
            Some(asr) => asr.try_events(),
            None => return,
        };
        for event in events {
            self.apply_asr_event(event);
        }
        self.settings_transcription.busy = self.recorder.asr_job.is_running();
        let precision = self.settings_transcription.precision;
        let action = self.settings_transcription.action.take();
        let Some(asr) = &self.asr else {
            return;
        };
        if precision != self.asr_precision_sent {
            self.asr_precision_sent = precision;
            asr.send(AsrCommand::CheckModel { precision });
        }
        match action {
            Some(ModelAction::Download) => asr.send(AsrCommand::DownloadModel { precision }),
            Some(ModelAction::Cancel) => asr.send(AsrCommand::CancelDownload),
            Some(ModelAction::Delete) => {
                self.asr_deleting = true;
                asr.send(AsrCommand::DeleteModel { precision });
            }
            None => {}
        }
    }

    /// Connects the capture worker and asks it for the device lists.
    pub fn enable_audio(&mut self, handle: AudioHandle) {
        handle.send(Command::ListDevices);
        self.recorder.audio = Some(handle);
        self.recorder.go_live();
        self.enable_library();
        // The mock folder belongs to someone else's profile; record under this user's.
        if self.recorder.destination == crate::screens::recorder::MOCK_DESTINATION {
            if let Some(home) = std::env::var_os("USERPROFILE") {
                self.recorder.destination = std::path::PathBuf::from(home)
                    .join("Documents")
                    .join("MeetingRecorder")
                    .join("Recordings")
                    .to_string_lossy()
                    .into_owned();
            }
        }
    }

    /// Makes Recordings and Transcripts list the folders under the Recorder's destination.
    pub fn enable_library(&mut self) {
        self.library_live = true;
        self.recordings.go_live();
        self.refresh_library();
    }

    /// Starts a background scan of the destination folder (no-op in preview mode).
    pub fn refresh_library(&mut self) {
        if self.library_live {
            self.library
                .refresh(std::path::PathBuf::from(&self.recorder.destination));
        }
    }

    /// Per frame: refresh when Recordings or Transcripts is opened, take a finished scan,
    /// show which folder is being transcribed, and run row button requests.
    fn sync_library(&mut self, ctx: &egui::Context) {
        if !self.library_live {
            return;
        }
        if self.current_screen != self.last_screen {
            self.last_screen = self.current_screen;
            if matches!(
                self.current_screen,
                Screen::Recordings | Screen::Transcripts
            ) {
                self.refresh_library();
            }
        }
        if let Some(list) = self.library.poll() {
            self.recordings.set_entries(list);
        }
        let processing = match (&self.recorder.asr_job, &self.recorder.transcript_dir) {
            (AsrJob::Waiting, Some(dir)) => Some((dir.clone(), 0)),
            (
                AsrJob::Running {
                    done_secs,
                    total_secs,
                },
                Some(dir),
            ) => {
                let pct = if *total_secs <= 0.0 {
                    0
                } else {
                    (done_secs / total_secs * 100.0).clamp(0.0, 99.0) as u8
                };
                Some((dir.clone(), pct))
            }
            _ => None,
        };
        self.recordings.set_processing(processing);
        if let Some(request) = self.recordings.request.take() {
            self.handle_recordings_request(request);
        }
        if self.library.is_pending() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn handle_recordings_request(&mut self, request: RecordingsRequest) {
        match request {
            RecordingsRequest::OpenTranscript(_dir) => {
                self.current_screen = Screen::TranscriptDetail;
            }
            RecordingsRequest::Transcribe(dir) => self.start_transcription(dir),
            RecordingsRequest::OpenFolder(dir) => open_in_explorer(&dir),
        }
    }

    /// Deletes a recording folder. Refused while a recording is in progress or while the folder
    /// is being transcribed.
    pub fn delete_recording(&mut self, dir: &std::path::Path) -> Result<(), String> {
        if self.recorder.state.session_active() {
            return Err("Stop the recording before deleting.".to_owned());
        }
        let busy = matches!(
            self.recorder.asr_job,
            AsrJob::Waiting | AsrJob::Running { .. }
        ) && self.recorder.transcript_dir.as_deref() == Some(dir);
        if busy {
            return Err("This recording is being transcribed.".to_owned());
        }
        let base = std::path::PathBuf::from(&self.recorder.destination);
        crate::library::delete_session(&base, dir)
            .map_err(|e| format!("Could not delete the recording: {e}"))?;
        self.refresh_library();
        Ok(())
    }

    /// Deletes what the delete modal named: a real folder, or a sample row in preview mode.
    pub fn apply_delete(&mut self, id: usize, dir: Option<std::path::PathBuf>) {
        match dir {
            Some(dir) => {
                if let Err(message) = self.delete_recording(&dir) {
                    self.asr_notice = Some(message);
                }
            }
            None => {
                self.recordings.recordings.retain(|r| r.id != id);
                if self.recordings.playback_id == Some(id) {
                    self.recordings.playback_id = None;
                }
            }
        }
    }

    /// Hands the capture settings to the Recorder (it reads them when Start is pressed).
    fn sync_audio_settings(&mut self) {
        self.recorder.sync_capture(
            crate::screens::settings::audio::sample_rate_hz(self.settings_audio.sample_rate_idx),
            self.audio_choice.system_device(),
            self.audio_choice.mic_device(),
        );
    }

    /// Applies one report from the capture worker.
    pub fn apply_audio_event(&mut self, event: Event) {
        match event {
            Event::Devices { system, mic } => self.audio_choice.apply_devices(system, mic),
            Event::Saved { dir, duration } => {
                self.recorder.apply_event(Event::Saved {
                    dir: dir.clone(),
                    duration,
                });
                self.start_transcription(dir);
                self.refresh_library();
            }
            other => self.recorder.apply_event(other),
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
        self.sync_audio_settings();
        for event in self.recorder.take_events() {
            self.apply_audio_event(event);
        }
        if let Some(notice) = self.recorder.take_notice() {
            crate::toast::show(ctx, notice);
        }
        self.sync_asr();
        self.sync_library(ctx);
        if let Some(notice) = self.take_asr_notice() {
            crate::toast::show(ctx, crate::i18n::tr(&notice));
        }
        if self.asr.is_some() {
            // Worker events do not wake egui by themselves.
            let busy = self.recorder.asr_job.is_running()
                || matches!(
                    self.recorder.engine,
                    EngineState::Downloading | EngineState::Loading
                );
            let wait = if busy { 100 } else { 500 };
            ctx.request_repaint_after(std::time::Duration::from_millis(wait));
        }
        if self.recorder.audio.is_some()
            && !matches!(
                self.recorder.state,
                crate::screens::recorder::RecorderState::Idle
                    | crate::screens::recorder::RecorderState::Saved
                    | crate::screens::recorder::RecorderState::Error
            )
        {
            // Worker events do not wake egui by themselves.
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        if self.recorder.audio.is_some() && !self.audio_choice.devices_loaded {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
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
            if let Some(Modal::DeleteRecording { id, dir, .. }) = self.open_modal.clone() {
                self.apply_delete(id, dir);
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

/// Shows a folder in Windows Explorer.
fn open_in_explorer(dir: &std::path::Path) {
    let _ = std::process::Command::new("explorer").arg(dir).spawn();
}

// ─── Main content area ───────────────────────────────────────────────────────

fn draw_content(ui: &mut Ui, state: &mut AppState, theme: &ThemeColors) {
    match state.current_screen {
        Screen::Recorder => {
            draw_recorder_screen(ui, theme, &mut state.recorder, &mut state.audio_choice);
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
        Screen::SettingsAudio => draw_settings_audio(
            ui,
            &mut state.settings_audio,
            &mut state.audio_choice,
            theme,
        ),
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

    struct NoModel;
    impl crate::asr::worker::RecognizerFactory for NoModel {
        fn load(
            &self,
            _dir: &std::path::Path,
            _threads: usize,
        ) -> Result<Box<dyn crate::asr::engine::Recognizer>, String> {
            Err("unused".into())
        }
    }

    fn asr_handle(models: &std::path::Path) -> crate::asr::worker::AsrHandle {
        crate::asr::worker::AsrHandle::spawn(
            std::sync::Arc::new(NoModel),
            crate::asr::worker::AsrConfig {
                models_base: Some(models.to_path_buf()),
                ..Default::default()
            },
        )
    }

    fn install(base: &std::path::Path, p: crate::asr::model::Precision) -> std::path::PathBuf {
        use crate::asr::model::{self, Precision};
        let dir = model::model_dir_in(base, p);
        std::fs::create_dir_all(&dir).unwrap();
        let (encoder, data) = match p {
            Precision::Int8 => ("encoder.int8.onnx", "encoder.int8.onnx.data"),
            Precision::Fp32 => ("encoder.onnx", "encoder.onnx.data"),
        };
        for f in [encoder, "decoder_joint.onnx", "tokens.txt"] {
            std::fs::write(dir.join(f), b"x").unwrap();
        }
        std::fs::write(dir.join(data), b"weights").unwrap();
        model::mark_ready(&dir, data).unwrap();
        dir
    }

    fn wait_for(a: &mut AppState, want: crate::asr::model::ModelStatus) {
        let t = std::time::Instant::now();
        while a.settings_transcription.model_status != want {
            a.sync_asr();
            assert!(
                t.elapsed() < std::time::Duration::from_secs(5),
                "never became {want:?}"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn model_events_drive_engine_and_model_status() {
        use crate::asr::model::ModelStatus as M;
        use crate::asr::worker::{EngineEvent, Event as E};
        use crate::screens::recorder::EngineState as S;
        let mut a = AppState::default();
        let cases = [
            (E::Model(M::Ready), S::Ready),
            (E::Model(M::Missing), S::Missing),
            (
                E::Model(M::Downloading {
                    done: 50,
                    total: 100,
                }),
                S::Downloading,
            ),
            (E::Model(M::Verifying), S::Loading),
            (E::Model(M::Failed("x".into())), S::Error),
            (E::Engine(EngineEvent::Loading), S::Loading),
            (E::Engine(EngineEvent::Ready), S::Ready),
            (E::Engine(EngineEvent::Failed("x".into())), S::Error),
        ];
        for (event, engine) in cases {
            let label = format!("{event:?}");
            a.apply_asr_event(event);
            assert_eq!(a.recorder.engine, engine, "{label}");
        }
        a.apply_asr_event(E::Model(M::Downloading {
            done: 50,
            total: 100,
        }));
        assert_eq!(
            a.settings_transcription.model_status,
            M::Downloading {
                done: 50,
                total: 100
            }
        );
        a.apply_asr_event(E::Model(M::Ready));
        assert_eq!(a.settings_transcription.model_status, M::Ready);
    }

    #[test]
    fn download_finishing_or_failing_raises_a_notice() {
        use crate::asr::model::ModelStatus as M;
        use crate::asr::worker::Event as E;
        let mut a = AppState::default();
        a.apply_asr_event(E::Model(M::Downloading { done: 1, total: 2 }));
        assert_eq!(a.take_asr_notice(), None);
        a.apply_asr_event(E::Model(M::Ready));
        assert_eq!(a.take_asr_notice().as_deref(), Some("Model downloaded"));
        assert_eq!(a.take_asr_notice(), None, "taken once");
        // a plain check that finds the model is not news
        a.apply_asr_event(E::Model(M::Ready));
        assert_eq!(a.take_asr_notice(), None);
        a.apply_asr_event(E::Model(M::Downloading { done: 1, total: 2 }));
        a.apply_asr_event(E::Model(M::Failed("cancelled".into())));
        assert_eq!(a.take_asr_notice().as_deref(), Some("Download failed"));
    }

    #[test]
    fn snapshot_carries_precision_and_threads() {
        let mut a = AppState::default();
        a.settings_transcription.precision = crate::asr::model::Precision::Fp32;
        a.settings_transcription.threads_idx = 2;
        let p = a.snapshot();
        assert_eq!(p.asr_precision, "fp32");
        assert_eq!(p.asr_threads, 8);

        let mut b = AppState::default();
        b.enable_persistence(p);
        assert_eq!(
            b.settings_transcription.precision,
            crate::asr::model::Precision::Fp32
        );
        assert_eq!(b.settings_transcription.threads(), 8);
    }

    #[test]
    fn changing_precision_asks_the_worker_about_the_new_model() {
        use crate::asr::model::{ModelStatus, Precision};
        use crate::screens::recorder::EngineState;
        let base = crate::audio::temp_dir("app-precision");
        install(&base, Precision::Fp32); // only fp32 is installed

        let mut a = AppState::default();
        a.enable_asr(asr_handle(&base));
        wait_for(&mut a, ModelStatus::Missing); // int8 is the default
        assert_eq!(a.recorder.engine, EngineState::Missing);

        a.settings_transcription.precision = Precision::Fp32;
        wait_for(&mut a, ModelStatus::Ready);
        assert_eq!(a.recorder.engine, EngineState::Ready);
        drop(a);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn delete_button_reaches_the_worker() {
        use crate::asr::model::{ModelStatus, Precision};
        use crate::screens::settings::transcription::ModelAction;
        let base = crate::audio::temp_dir("app-delete");
        let dir = install(&base, Precision::Int8);

        let mut a = AppState::default();
        a.enable_asr(asr_handle(&base));
        wait_for(&mut a, ModelStatus::Ready);
        a.settings_transcription.action = Some(ModelAction::Delete);
        wait_for(&mut a, ModelStatus::Missing);
        assert!(!dir.exists());
        assert_eq!(a.take_asr_notice().as_deref(), Some("Model deleted"));
        assert!(
            a.settings_transcription.action.is_none(),
            "action taken once"
        );
        drop(a);
        let _ = std::fs::remove_dir_all(base);
    }

    struct Says(&'static str);
    struct SaysRecognizer(&'static str);
    impl crate::asr::engine::Recognizer for SaysRecognizer {
        fn recognize(&mut self, _samples: &[f32]) -> Result<String, String> {
            Ok(self.0.to_owned())
        }
    }
    impl crate::asr::worker::RecognizerFactory for Says {
        fn load(
            &self,
            _dir: &std::path::Path,
            _threads: usize,
        ) -> Result<Box<dyn crate::asr::engine::Recognizer>, String> {
            Ok(Box::new(SaysRecognizer(self.0)))
        }
    }

    fn says_handle(models: &std::path::Path) -> crate::asr::worker::AsrHandle {
        crate::asr::worker::AsrHandle::spawn(
            std::sync::Arc::new(Says("hi")),
            crate::asr::worker::AsrConfig {
                models_base: Some(models.to_path_buf()),
                ..Default::default()
            },
        )
    }

    /// A session folder with 2 s of loud audio on both tracks.
    fn session_dir(base: &std::path::Path, name: &str) -> std::path::PathBuf {
        let dir = base.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let tone: Vec<i16> = (0..32_000)
            .map(|i| (0.3 * 32_767.0 * (i as f32 * 0.07).sin()) as i16)
            .collect();
        for f in ["mic.wav", "system.wav"] {
            let file = std::fs::File::create(dir.join(f)).unwrap();
            let mut w = crate::audio::wav::WavWriter::new(file, 16_000).unwrap();
            w.write_samples(&tone).unwrap();
            w.finish().unwrap();
        }
        dir
    }

    fn saved(dir: &std::path::Path) -> Event {
        Event::Saved {
            dir: dir.to_path_buf(),
            duration: std::time::Duration::from_secs(2),
        }
    }

    fn wait_until(a: &mut AppState, what: &str, done: impl Fn(&AppState) -> bool) {
        let t = std::time::Instant::now();
        while !done(a) {
            a.sync_asr();
            assert!(
                t.elapsed() < std::time::Duration::from_secs(10),
                "timed out waiting for {what}"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn saved_recording_is_transcribed_into_the_panel() {
        use crate::asr::model::{ModelStatus, Precision};
        use crate::asr::transcript::Track;
        use crate::screens::recorder::AsrJob;
        let base = crate::audio::temp_dir("app-job");
        install(&base, Precision::Int8);
        let dir = session_dir(&base, "rec1");

        let mut a = AppState::default();
        a.enable_asr(says_handle(&base));
        wait_for(&mut a, ModelStatus::Ready);
        a.apply_audio_event(saved(&dir));
        assert!(matches!(a.recorder.asr_job, AsrJob::Running { .. }));
        assert_eq!(a.recorder.transcript_dir.as_deref(), Some(dir.as_path()));

        wait_until(&mut a, "job done", |a| a.recorder.asr_job == AsrJob::Done);
        let speakers: Vec<_> = a
            .recorder
            .transcript_segments
            .iter()
            .map(|s| s.speaker)
            .collect();
        assert_eq!(speakers, vec![Some(Track::Mic), Some(Track::System)]);
        assert!(a
            .recorder
            .transcript_segments
            .iter()
            .all(|s| s.text == "hi"));
        assert!(dir.join("transcript.json").is_file());
        assert!(dir.join("transcript.txt").is_file());
        drop(a);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn saved_without_the_model_waits_and_leaves_the_recording_alone() {
        use crate::asr::model::{ModelStatus, Precision};
        use crate::screens::recorder::{AsrJob, AsrRequest, RecorderState};
        let base = crate::audio::temp_dir("app-wait");
        let dir = session_dir(&base, "rec1");

        let mut a = AppState::default();
        a.enable_asr(says_handle(&base));
        wait_for(&mut a, ModelStatus::Missing);
        a.recorder.state = RecorderState::Saved;
        a.apply_audio_event(saved(&dir));
        assert_eq!(a.recorder.asr_job, AsrJob::Waiting);
        assert_eq!(a.recorder.state, RecorderState::Saved);
        assert_eq!(
            a.take_asr_notice().as_deref(),
            Some("Waiting for the model. The recording is saved; transcribe it after the model is downloaded.")
        );
        assert!(!dir.join("transcript.json").exists(), "nothing was started");

        // the model arrives; the Transcribe button starts the job
        install(&base, Precision::Int8);
        a.apply_asr_event(crate::asr::worker::Event::Model(ModelStatus::Ready));
        a.recorder.request = Some(AsrRequest::Transcribe);
        a.sync_asr();
        assert!(matches!(a.recorder.asr_job, AsrJob::Running { .. }));
        wait_until(&mut a, "job done", |a| a.recorder.asr_job == AsrJob::Done);
        assert!(dir.join("transcript.txt").is_file());
        drop(a);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn two_saved_recordings_both_get_a_transcript() {
        use crate::asr::model::{ModelStatus, Precision};
        use crate::screens::recorder::AsrJob;
        let base = crate::audio::temp_dir("app-two");
        install(&base, Precision::Int8);
        let (d1, d2) = (session_dir(&base, "rec1"), session_dir(&base, "rec2"));

        let mut a = AppState::default();
        a.enable_asr(says_handle(&base));
        wait_for(&mut a, ModelStatus::Ready);
        a.apply_audio_event(saved(&d1));
        a.apply_audio_event(saved(&d2));
        assert_eq!(
            a.recorder.transcript_dir.as_deref(),
            Some(d1.as_path()),
            "the panel shows the first job"
        );
        wait_until(&mut a, "second job done", |a| {
            a.recorder.transcript_dir.as_deref() == Some(d2.as_path())
                && a.recorder.asr_job == AsrJob::Done
        });
        assert!(d1.join("transcript.txt").is_file());
        assert!(d2.join("transcript.txt").is_file());
        drop(a);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn a_failing_job_shows_its_message_and_not_the_recorder_error() {
        use crate::asr::model::{ModelStatus, Precision};
        use crate::screens::recorder::AsrJob;
        let base = crate::audio::temp_dir("app-fail");
        install(&base, Precision::Int8);
        let dir = base.join("empty"); // no recordings in it
        std::fs::create_dir_all(&dir).unwrap();

        let mut a = AppState::default();
        a.enable_asr(says_handle(&base));
        wait_for(&mut a, ModelStatus::Ready);
        a.apply_audio_event(saved(&dir));
        wait_until(&mut a, "failure", |a| {
            matches!(a.recorder.asr_job, AsrJob::Failed(_))
        });
        assert!(a
            .recorder
            .asr_failure_text()
            .unwrap()
            .starts_with("No recordings found"));
        assert!(a.recorder.last_error.is_none());
        drop(a);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn cancel_button_stops_the_job() {
        use crate::asr::model::{ModelStatus, Precision};
        use crate::screens::recorder::{AsrJob, AsrRequest};
        let base = crate::audio::temp_dir("app-cancel");
        install(&base, Precision::Int8);
        let dir = session_dir(&base, "rec1");

        let mut a = AppState::default();
        a.enable_asr(says_handle(&base));
        wait_for(&mut a, ModelStatus::Ready);
        a.apply_audio_event(saved(&dir));
        a.recorder.request = Some(AsrRequest::Cancel);
        // the job may already be over on a fast machine; either way the app must settle
        wait_until(&mut a, "settled", |a| {
            matches!(a.recorder.asr_job, AsrJob::Idle | AsrJob::Done)
        });
        drop(a);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn settings_button_in_the_recorder_opens_transcription_settings() {
        use crate::screens::recorder::AsrRequest;
        let mut a = AppState::default();
        a.recorder.request = Some(AsrRequest::OpenSettings);
        a.sync_asr();
        assert_eq!(a.current_screen, Screen::SettingsTranscription);
    }

    #[test]
    fn preview_mode_keeps_the_mock_transcript_when_saved_is_simulated() {
        use crate::screens::recorder::AsrJob;
        let mut a = AppState::default();
        a.apply_audio_event(saved(std::path::Path::new("somewhere")));
        assert_eq!(a.recorder.transcript_segments.len(), 5);
        assert_eq!(a.recorder.asr_job, AsrJob::Idle);
        assert!(!a.recorder.live);
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

    #[test]
    fn devices_event_fills_lists() {
        use crate::audio::session::{DeviceInfo, Event};
        let dev = |id: &str| DeviceInfo {
            id: id.to_owned(),
            name: id.to_owned(),
            is_default: false,
        };
        let mut state = AppState::default();
        state.audio_choice.mic_id = "gone".to_owned();
        assert!(!state.audio_choice.devices_loaded);
        state.apply_audio_event(Event::Devices {
            system: vec![dev("s1")],
            mic: vec![dev("m1")],
        });
        assert!(state.audio_choice.devices_loaded);
        assert_eq!(state.audio_choice.system.len(), 1);
        assert_eq!(
            state.audio_choice.mic_index(),
            0,
            "unknown id shows Windows default"
        );
        assert_eq!(
            state.audio_choice.mic_id, "gone",
            "stored id is not overwritten"
        );
    }

    #[test]
    fn device_choice_is_part_of_the_saved_snapshot() {
        let mut state = AppState::default();
        state.enable_persistence(Persisted {
            system_device: "s".to_owned(),
            mic_device: "m".to_owned(),
            ..Persisted::default()
        });
        assert_eq!(state.audio_choice.system_id, "s");
        assert_eq!(state.snapshot().mic_device, "m");
    }

    /// Applies worker events until `done` holds (max 5 s).
    fn pump(state: &mut AppState, done: impl Fn(&AppState) -> bool) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !done(state) {
            for e in state.recorder.take_events() {
                state.apply_audio_event(e);
            }
            assert!(
                std::time::Instant::now() < deadline,
                "timed out; state {:?}",
                state.recorder.state
            );
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    #[test]
    fn stop_shortcut_in_session_sends_stop_command() {
        use crate::audio::backend::FakeBackend;
        use crate::audio::worker::{AudioHandle, WorkerConfig};
        use crate::screens::recorder::RecorderState;
        let dir = crate::audio::temp_dir("app-session");
        let mut state = AppState::default();
        state.recorder.destination = dir.to_string_lossy().into_owned();
        state.recorder.file_name = "shortcut".to_owned();
        state.enable_audio(AudioHandle::spawn(
            std::sync::Arc::new(FakeBackend::new()),
            WorkerConfig::default(),
        ));
        // Ctrl+Shift+R starts ...
        state.recorder.toggle_recording();
        pump(&mut state, |s| s.recorder.state == RecorderState::Recording);
        // ... and the same shortcut stops: the worker finalizes and reports Saved.
        state.recorder.toggle_recording();
        pump(&mut state, |s| s.recorder.state == RecorderState::Saved);
        let saved = state.recorder.saved_dir.clone().expect("saved folder");
        assert!(saved.join("system.wav").is_file() && saved.join("mic.wav").is_file());
    }

    fn library_dir(name: &str) -> std::path::PathBuf {
        crate::audio::temp_dir(name)
    }

    fn pump_library(a: &mut AppState, what: &str, done: impl Fn(&AppState) -> bool) {
        let ctx = egui::Context::default();
        let t = std::time::Instant::now();
        while !done(a) {
            a.sync_library(&ctx);
            assert!(
                t.elapsed() < std::time::Duration::from_secs(10),
                "timed out waiting for {what}"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn delete_is_refused_while_recording() {
        use crate::screens::recorder::RecorderState;
        let base = library_dir("app-del-rec");
        let dir = session_dir(&base, "2026-11-24_1000_a");
        let mut a = AppState::default();
        a.recorder.destination = base.to_string_lossy().into_owned();
        a.recorder.state = RecorderState::Recording;
        assert!(a.delete_recording(&dir).is_err());
        assert!(dir.join("mic.wav").is_file(), "nothing was deleted");
        a.recorder.state = RecorderState::Idle;
        assert!(a.delete_recording(&dir).is_ok());
        assert!(!dir.exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn delete_is_refused_while_the_folder_is_transcribing() {
        let base = library_dir("app-del-job");
        let busy = session_dir(&base, "2026-11-24_1000_a");
        let other = session_dir(&base, "2026-11-24_1001_b");
        let mut a = AppState::default();
        a.recorder.destination = base.to_string_lossy().into_owned();
        a.recorder.asr_job = AsrJob::Running {
            done_secs: 1.0,
            total_secs: 10.0,
        };
        a.recorder.transcript_dir = Some(busy.clone());
        assert!(a.delete_recording(&busy).is_err());
        assert!(busy.join("mic.wav").is_file());
        assert!(a.delete_recording(&other).is_ok(), "another folder is fine");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn deleting_through_the_modal_path_removes_the_folder_and_the_row() {
        let base = library_dir("app-del-row");
        let dir = session_dir(&base, "2026-11-24_1000_a");
        session_dir(&base, "2026-11-24_1001_b");
        let mut a = AppState::default();
        a.recorder.destination = base.to_string_lossy().into_owned();
        a.enable_library();
        pump_library(&mut a, "two rows", |a| a.recordings.recordings.len() == 2);
        a.apply_delete(1, Some(dir.clone()));
        assert!(!dir.exists());
        pump_library(&mut a, "one row", |a| a.recordings.recordings.len() == 1);
        assert_eq!(a.recordings.recordings[0].display_name, "b");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_failed_delete_is_reported_and_keeps_the_row() {
        let base = library_dir("app-del-fail");
        let outside = library_dir("app-del-outside");
        let dir = session_dir(&outside, "2026-11-24_1000_a");
        let mut a = AppState::default();
        a.recorder.destination = base.to_string_lossy().into_owned();
        a.apply_delete(1, Some(dir.clone()));
        assert!(a.take_asr_notice().is_some());
        assert!(
            dir.join("mic.wav").is_file(),
            "outside the destination: untouched"
        );
        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn the_list_refreshes_when_opened_and_after_a_save() {
        let base = library_dir("app-refresh");
        let dir = session_dir(&base, "2026-11-24_1000_a");
        let mut a = AppState::default();
        a.recorder.destination = base.to_string_lossy().into_owned();
        let ctx = egui::Context::default();
        a.sync_library(&ctx);
        assert_eq!(a.library.requested(), 0, "preview mode never scans");
        a.enable_library();
        assert_eq!(a.library.requested(), 1);
        a.current_screen = Screen::Recordings;
        a.sync_library(&ctx);
        assert_eq!(a.library.requested(), 2, "opening Recordings refreshes");
        a.sync_library(&ctx);
        assert_eq!(a.library.requested(), 2, "only once per visit");
        a.apply_audio_event(saved(&dir));
        assert_eq!(a.library.requested(), 3, "a saved recording refreshes");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_new_folder_appears_without_restarting() {
        let base = library_dir("app-newfolder");
        let mut a = AppState::default();
        a.recorder.destination = base.to_string_lossy().into_owned();
        a.enable_library();
        pump_library(&mut a, "first scan", |a| a.recordings.loaded);
        assert!(a.recordings.recordings.is_empty());
        session_dir(&base, "2026-11-24_1000_made-in-explorer");
        a.current_screen = Screen::Recordings;
        pump_library(&mut a, "the new row", |a| {
            a.recordings.recordings.len() == 1
        });
        let _ = std::fs::remove_dir_all(&base);
    }
}
