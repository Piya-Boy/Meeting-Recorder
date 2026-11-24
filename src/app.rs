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
    transcripts::{draw_transcripts_screen, TranscriptsRequest, TranscriptsScreen},
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
    /// The recording being played, if any (the playback thread lives inside).
    player: Option<crate::playback::PlayerHandle>,
    /// Starts a player on a folder; tests replace it with one that uses a fake device.
    player_start: fn(&std::path::Path) -> crate::playback::PlayerHandle,

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
            player: None,
            player_start: crate::playback::PlayerHandle::start,
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
        self.transcripts.go_live();
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
            self.transcripts.set_entries(&list);
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
        if let Some(request) = self.transcripts.request.take() {
            self.handle_transcripts_request(ctx, request);
        }
        if self.library.is_pending() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn handle_recordings_request(&mut self, request: RecordingsRequest) {
        match request {
            RecordingsRequest::OpenTranscript(dir) => self.open_transcript(&dir),
            RecordingsRequest::Transcribe(dir) => self.start_transcription(dir),
            RecordingsRequest::OpenFolder(dir) => open_in_explorer(&dir),
            RecordingsRequest::TogglePlay(dir) => self.toggle_play(dir),
            RecordingsRequest::Seek(fraction) => {
                if let Some(p) = &self.player {
                    let total = p.snapshot().total_frames as f64;
                    let frame = (f64::from(fraction.clamp(0.0, 1.0)) * total) as u64;
                    p.send(crate::playback::Cmd::Seek(frame));
                }
            }
            RecordingsRequest::StopPlayback => self.stop_player(),
        }
    }

    /// Row or bar play button: pauses or resumes the recording that is loaded, otherwise loads
    /// this one (replacing the old player and closing its files first).
    pub fn toggle_play(&mut self, dir: std::path::PathBuf) {
        use crate::playback::{Cmd, State};
        if self.recorder.state.session_active() {
            self.asr_notice = Some("Stop recording before playing.".to_owned());
            return;
        }
        if let Some(p) = &self.player {
            if p.dir() == dir.as_path() {
                match p.snapshot().state {
                    State::Playing => return p.send(Cmd::Pause),
                    State::Paused | State::Finished => return p.send(Cmd::Resume),
                    State::Failed => {} // start over below
                }
            }
        }
        self.stop_player();
        self.player = Some((self.player_start)(&dir));
    }

    /// Stops playback and waits until the player has closed the files.
    pub fn stop_player(&mut self) {
        if let Some(p) = self.player.take() {
            p.stop();
        }
        self.recordings.player = None;
    }

    /// Per frame: copy the player's state to the Recordings screen, report a failed player, and
    /// stop playback when a recording starts (it would be recorded by a loopback source).
    pub fn sync_player(&mut self, ctx: &egui::Context) {
        use crate::playback::State;
        let Some(p) = &self.player else {
            self.recordings.player = None;
            return;
        };
        if self.recorder.state.session_active() {
            self.stop_player();
            self.asr_notice = Some("Playback stopped because a recording started.".to_owned());
            return;
        }
        let snap = p.snapshot();
        if snap.state == State::Failed {
            self.stop_player();
            self.asr_notice = Some("Could not play the recording.".to_owned());
            return;
        }
        let dir = p.dir().to_path_buf();
        let name = self
            .recordings
            .recordings
            .iter()
            .find(|r| r.dir.as_deref() == Some(dir.as_path()))
            .map(|r| r.display_name.clone())
            .or_else(|| dir.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default();
        let rate = snap.rate.max(1) as f32;
        self.recordings.player = Some(crate::screens::recordings::PlayerView {
            dir,
            name,
            playing: snap.state == State::Playing,
            position_s: snap.position_frames as f32 / rate,
            total_s: snap.total_frames as f32 / rate,
        });
        if snap.state == State::Playing {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn handle_transcripts_request(&mut self, ctx: &egui::Context, request: TranscriptsRequest) {
        match request {
            TranscriptsRequest::Open(dir) => self.open_transcript(&dir),
            TranscriptsRequest::Export(dir, format) => self.export_transcript(&dir, format),
            TranscriptsRequest::Copy(dir) => match crate::library::load_transcript(&dir) {
                Ok(lines) => {
                    ctx.copy_text(crate::screens::transcript_detail::lines_text(&lines));
                    self.asr_notice = Some("Transcript copied to clipboard".to_owned());
                }
                Err(_) => self.asr_notice = Some("Could not open the transcript.".to_owned()),
            },
        }
    }

    /// Writes the export file of a recording next to it and shows it in Explorer.
    pub fn export_transcript(
        &mut self,
        dir: &std::path::Path,
        format: crate::export::ExportFormat,
    ) {
        self.export_transcript_with(dir, format, reveal_in_explorer);
    }

    /// `export_transcript` with the "show the file" step passed in, so tests do not open Explorer.
    fn export_transcript_with(
        &mut self,
        dir: &std::path::Path,
        format: crate::export::ExportFormat,
        reveal: impl FnOnce(&std::path::Path),
    ) {
        use crate::i18n::tr;
        match crate::export::export_transcript(dir, format, tr("You"), tr("Others")) {
            Ok(path) => {
                reveal(&path);
                self.asr_notice =
                    Some("Exported. The file is in the recording's folder.".to_owned());
            }
            Err(_) => self.asr_notice = Some("Could not export the transcript.".to_owned()),
        }
    }

    /// Loads the transcript of a recording into the detail page and opens it. A damaged or
    /// missing file shows a notice and leaves the current screen as it is.
    pub fn open_transcript(&mut self, dir: &std::path::Path) {
        let lines = match crate::library::load_transcript(dir) {
            Ok(lines) => lines,
            Err(_) => {
                self.asr_notice = Some("Could not open the transcript.".to_owned());
                return;
            }
        };
        let row = self
            .recordings
            .recordings
            .iter()
            .find(|r| r.dir.as_deref() == Some(dir));
        let (title, date, duration) = match row {
            Some(r) => (r.display_name.clone(), r.date.clone(), r.duration.clone()),
            None => (
                dir.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                String::new(),
                String::new(),
            ),
        };
        self.transcript_detail
            .show_real(title, date, duration, dir.to_path_buf(), &lines);
        self.current_screen = Screen::TranscriptDetail;
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
        // The player holds the WAV files open, which would make the delete fail.
        if self.player.as_ref().is_some_and(|p| p.dir() == dir) {
            self.stop_player();
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
        self.sync_player(ctx);
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

/// The argument that makes Explorer open a folder with `path` selected: `/select,"C:\a b\c"`.
/// Explorer wants backslashes and the quotes inside one argument.
fn explorer_select_arg(path: &std::path::Path) -> String {
    format!(
        "/select,\"{}\"",
        path.display().to_string().replace('/', "\\")
    )
}

/// Shows a file in Windows Explorer, selected in its folder.
fn reveal_in_explorer(path: &std::path::Path) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("explorer")
            .raw_arg(explorer_select_arg(path))
            .spawn();
    }
    #[cfg(not(windows))]
    if let Some(dir) = path.parent() {
        open_in_explorer(dir);
    }
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

    fn write_real_transcript(dir: &std::path::Path) {
        use crate::asr::transcript::{Line, ModelInfo, Track, Transcript};
        let t = Transcript {
            model: ModelInfo {
                name: "m".into(),
                revision: "r".into(),
                precision: "int8".into(),
            },
            created: "2026-11-24T10:00:00".into(),
            lines: vec![
                Line {
                    track: Track::Mic,
                    start_ms: 0,
                    end_ms: 900,
                    text: "สวัสดีครับ".into(),
                },
                Line {
                    track: Track::System,
                    start_ms: 1500,
                    end_ms: 2400,
                    text: "เราจะทำงานนี้".into(),
                },
            ],
        };
        crate::asr::transcript::write(dir, &t, "You", "Others").unwrap();
    }

    #[test]
    fn a_scan_fills_both_lists_and_opening_a_transcript_shows_its_lines() {
        let base = library_dir("app-open");
        let done = session_dir(&base, "2026-11-24_1000_done");
        session_dir(&base, "2026-11-24_0900_audio-only");
        write_real_transcript(&done);
        let mut a = AppState::default();
        a.recorder.destination = base.to_string_lossy().into_owned();
        a.enable_library();
        pump_library(&mut a, "both lists", |a| {
            a.recordings.loaded && a.transcripts.loaded
        });
        assert_eq!(a.recordings.recordings.len(), 2);
        assert_eq!(
            a.transcripts.transcripts.len(),
            1,
            "only the transcribed one"
        );
        a.open_transcript(&done);
        assert_eq!(a.current_screen, Screen::TranscriptDetail);
        assert!(a.transcript_detail.live);
        assert_eq!(a.transcript_detail.title, "done");
        assert_eq!(a.transcript_detail.articles.len(), 2);
        assert_eq!(a.transcript_detail.articles[1].timestamp, "00:00:01");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_damaged_transcript_reports_and_stays_on_the_list() {
        let base = library_dir("app-damaged");
        let dir = session_dir(&base, "2026-11-24_1000_bad");
        std::fs::write(dir.join("transcript.json"), "{ not json").unwrap();
        let mut a = AppState {
            current_screen: Screen::Recordings,
            ..AppState::default()
        };
        a.open_transcript(&dir);
        assert_eq!(a.current_screen, Screen::Recordings);
        assert!(!a.transcript_detail.live, "the page keeps what it showed");
        assert!(a.take_asr_notice().is_some());
        a.open_transcript(&base.join("missing"));
        assert_eq!(a.current_screen, Screen::Recordings);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn the_transcripts_copy_button_reports_success_and_a_missing_file() {
        let base = library_dir("app-copy");
        let dir = session_dir(&base, "2026-11-24_1000_done");
        write_real_transcript(&dir);
        let mut a = AppState::default();
        let ctx = egui::Context::default();
        a.handle_transcripts_request(&ctx, TranscriptsRequest::Copy(dir.clone()));
        assert_eq!(
            a.take_asr_notice().as_deref(),
            Some("Transcript copied to clipboard")
        );
        a.handle_transcripts_request(&ctx, TranscriptsRequest::Copy(base.join("missing")));
        assert_eq!(
            a.take_asr_notice().as_deref(),
            Some("Could not open the transcript.")
        );
        let text = crate::screens::transcript_detail::lines_text(
            &crate::library::load_transcript(&dir).unwrap(),
        );
        assert_eq!(text.lines().count(), 2);
        assert!(text.contains("] ") && text.contains("เราจะทำงานนี้"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn exporting_a_real_transcript_reports_success_and_a_damaged_one_failure() {
        let base = library_dir("app-export");
        let dir = session_dir(&base, "2026-11-25_1000_done");
        write_real_transcript(&dir);
        let mut a = AppState::default();
        let mut revealed: Vec<std::path::PathBuf> = Vec::new();
        a.export_transcript_with(&dir, crate::export::ExportFormat::Srt, |p| {
            revealed.push(p.to_path_buf())
        });
        assert_eq!(
            a.take_asr_notice().as_deref(),
            Some("Exported. The file is in the recording's folder.")
        );
        assert!(dir.join("transcript.srt").is_file());
        assert_eq!(revealed, vec![dir.join("transcript.srt")]);

        let bad = session_dir(&base, "2026-11-25_1001_bad");
        std::fs::write(bad.join("transcript.json"), "{ not json").unwrap();
        revealed.clear();
        a.export_transcript_with(&bad, crate::export::ExportFormat::Txt, |p| {
            revealed.push(p.to_path_buf())
        });
        assert_eq!(
            a.take_asr_notice().as_deref(),
            Some("Could not export the transcript.")
        );
        assert!(revealed.is_empty(), "nothing to show after a failure");
        assert!(!bad.join("transcript.txt").exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn explorer_select_argument_quotes_the_path_and_uses_backslashes() {
        let arg = explorer_select_arg(std::path::Path::new("C:/My Rec/2026 x/transcript.srt"));
        assert_eq!(arg, "/select,\"C:\\My Rec\\2026 x\\transcript.srt\"");
    }

    // ─── Playback (fake output device, real files) ───────────────────────────────────────────

    /// A session folder with a 10-second mic track, long enough to still be playing mid-test.
    fn play_dir(base: &std::path::Path, name: &str) -> std::path::PathBuf {
        let dir = base.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let ramp: Vec<i16> = (0..160_000)
            .map(|i| ((i % 2_000) as i16 - 1_000) * 10)
            .collect();
        let file = std::fs::File::create(dir.join("mic.wav")).unwrap();
        let mut w = crate::audio::wav::WavWriter::new(file, 16_000).unwrap();
        w.write_samples(&ramp).unwrap();
        w.finish().unwrap();
        dir
    }

    fn fake_app(base: &std::path::Path) -> AppState {
        let mut a = AppState {
            player_start: crate::playback::testing::start_fake,
            ..AppState::default()
        };
        a.recorder.destination = base.to_string_lossy().into_owned();
        a
    }

    fn failing_start(dir: &std::path::Path) -> crate::playback::PlayerHandle {
        crate::playback::PlayerHandle::start_with(dir, |_| Err("no output device".to_owned()))
    }

    fn wait_player(
        a: &AppState,
        what: &str,
        cond: impl Fn(&crate::playback::Snapshot) -> bool,
    ) -> crate::playback::Snapshot {
        let t = std::time::Instant::now();
        loop {
            let snap = a.player.as_ref().expect("a player").snapshot();
            if cond(&snap) {
                return snap;
            }
            assert!(
                t.elapsed() < std::time::Duration::from_secs(10),
                "timed out waiting for {what}: {snap:?}"
            );
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    #[test]
    fn toggle_play_starts_pauses_and_resumes_the_same_folder() {
        use crate::playback::State;
        let base = library_dir("app-play");
        let dir = play_dir(&base, "2026-11-25_1000_talk");
        let mut a = fake_app(&base);
        let ctx = egui::Context::default();

        a.toggle_play(dir.clone());
        wait_player(&a, "playing", |s| {
            s.state == State::Playing && s.position_frames > 0
        });
        a.sync_player(&ctx);
        let v = a.recordings.player.clone().expect("the bar has a player");
        assert_eq!(v.dir, dir);
        assert_eq!(v.name, "2026-11-25_1000_talk", "no row: the folder name");
        assert!(v.playing && v.position_s > 0.0);
        assert!((v.total_s - 10.0).abs() < 0.01, "{}", v.total_s);

        a.toggle_play(dir.clone());
        let paused = wait_player(&a, "paused", |s| s.state == State::Paused);
        a.sync_player(&ctx);
        assert!(!a.recordings.player.as_ref().unwrap().playing);
        std::thread::sleep(std::time::Duration::from_millis(30));
        let still = a.player.as_ref().unwrap().snapshot().position_frames;
        assert_eq!(still, paused.position_frames, "paused holds still");

        a.toggle_play(dir.clone());
        wait_player(&a, "playing again", |s| {
            s.state == State::Playing && s.position_frames > paused.position_frames
        });
        a.stop_player();
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn toggling_another_folder_replaces_the_player_and_closes_the_old_files() {
        use crate::playback::State;
        let base = library_dir("app-play-two");
        let first = play_dir(&base, "2026-11-25_1000_a");
        let second = play_dir(&base, "2026-11-25_1100_b");
        let mut a = fake_app(&base);
        a.toggle_play(first.clone());
        wait_player(&a, "first playing", |s| s.state == State::Playing);
        a.toggle_play(second.clone());
        assert_eq!(a.player.as_ref().unwrap().dir(), second.as_path());
        std::fs::remove_dir_all(&first).expect("the old player closed its files");
        a.stop_player();
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn play_is_refused_while_recording_and_playback_stops_when_a_recording_starts() {
        use crate::playback::State;
        use crate::screens::recorder::RecorderState;
        let base = library_dir("app-play-rec");
        let dir = play_dir(&base, "2026-11-25_1000_a");
        let mut a = fake_app(&base);
        let ctx = egui::Context::default();

        a.recorder.state = RecorderState::Recording;
        a.toggle_play(dir.clone());
        assert!(a.player.is_none());
        assert_eq!(
            a.take_asr_notice().as_deref(),
            Some("Stop recording before playing.")
        );

        a.recorder.state = RecorderState::Idle;
        a.toggle_play(dir.clone());
        wait_player(&a, "playing", |s| s.state == State::Playing);
        a.recorder.state = RecorderState::Recording;
        a.sync_player(&ctx);
        assert!(a.player.is_none() && a.recordings.player.is_none());
        assert_eq!(
            a.take_asr_notice().as_deref(),
            Some("Playback stopped because a recording started.")
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_failed_player_clears_the_bar_and_reports_it() {
        use crate::playback::State;
        let base = library_dir("app-play-fail");
        let dir = play_dir(&base, "2026-11-25_1000_a");
        let mut a = fake_app(&base);
        a.player_start = failing_start;
        let ctx = egui::Context::default();
        a.toggle_play(dir);
        wait_player(&a, "failure", |s| s.state == State::Failed);
        a.sync_player(&ctx);
        assert!(a.player.is_none() && a.recordings.player.is_none());
        assert_eq!(
            a.take_asr_notice().as_deref(),
            Some("Could not play the recording.")
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn delete_stops_the_player_first_and_removes_the_folder() {
        use crate::playback::State;
        let base = library_dir("app-play-del");
        let dir = play_dir(&base, "2026-11-25_1000_a");
        let mut a = fake_app(&base);
        a.toggle_play(dir.clone());
        wait_player(&a, "playing", |s| s.state == State::Playing);
        a.delete_recording(&dir)
            .expect("deleting the playing recording works");
        assert!(!dir.exists());
        assert!(a.player.is_none());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn seek_and_stop_requests_reach_the_player() {
        use crate::playback::State;
        let base = library_dir("app-play-seek");
        let dir = play_dir(&base, "2026-11-25_1000_a");
        let mut a = fake_app(&base);
        let ctx = egui::Context::default();
        a.toggle_play(dir.clone());
        wait_player(&a, "playing", |s| s.state == State::Playing);
        a.toggle_play(dir.clone());
        wait_player(&a, "paused", |s| s.state == State::Paused);
        a.handle_recordings_request(RecordingsRequest::Seek(0.5));
        wait_player(&a, "seek", |s| s.position_frames == 80_000);
        a.sync_player(&ctx);
        assert!((a.recordings.player.as_ref().unwrap().position_s - 5.0).abs() < 0.01);

        a.handle_recordings_request(RecordingsRequest::StopPlayback);
        a.sync_player(&ctx);
        assert!(a.player.is_none() && a.recordings.player.is_none());
        let _ = std::fs::remove_dir_all(&base);
    }
}
