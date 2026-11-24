use crate::i18n::tr;
use egui::{Color32, RichText, Ui};

use crate::components::audio_meter::audio_level_meter;
use crate::components::button::{block_button, custom_button, ButtonVariant};
use crate::components::toggle::toggle_switch;
use crate::theme::ThemeColors;

use crate::asr::transcript::{format_clock, Line, Track, Transcript};
use crate::audio::session::{Command, Event, SessionState, Source, SourceStatus, StartRequest};
use crate::audio::worker::AudioHandle;
use crate::screens::settings::audio::{device_dropdown, AudioChoice};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RecorderState {
    #[default]
    Idle,
    /// Start requested; the worker has not reported a running session yet.
    Starting,
    Recording,
    Paused,
    /// Stopping or finalizing the files.
    Processing,
    /// The files are written and closed.
    Saved,
    Error,
}

impl RecorderState {
    pub fn label(self) -> &'static str {
        match self {
            RecorderState::Idle => tr("Idle"),
            RecorderState::Starting => tr("Starting"),
            RecorderState::Recording => tr("Recording"),
            RecorderState::Paused => tr("Paused"),
            RecorderState::Processing => tr("Processing"),
            RecorderState::Saved => tr("Saved"),
            RecorderState::Error => tr("Error"),
        }
    }

    /// A session exists (or is being set up or torn down); another Start must not be sent.
    pub(crate) fn session_active(self) -> bool {
        matches!(
            self,
            RecorderState::Starting
                | RecorderState::Recording
                | RecorderState::Paused
                | RecorderState::Processing
        )
    }
}

/// `hh:mm:ss`.
fn clock(d: Duration) -> String {
    let s = d.as_secs();
    format!("{:02}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60)
}

/// Transcription engine readiness. `Missing` and `Downloading` are about the model files;
/// `Loading`, `Ready` and `Error` keep their meaning from the preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EngineState {
    /// The model is not installed.
    Missing,
    /// The model is being downloaded.
    Downloading,
    Loading,
    #[default]
    Ready,
    Error,
}

/// The transcription of the recording the right panel shows.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum AsrJob {
    #[default]
    Idle,
    /// The recording is saved but the model is not installed yet.
    Waiting,
    /// Sent to the worker; `total_secs` is 0 until the first progress report.
    Running {
        done_secs: f32,
        total_secs: f32,
    },
    Done,
    Failed(String),
}

impl AsrJob {
    pub fn is_running(&self) -> bool {
        matches!(self, AsrJob::Running { .. })
    }
}

/// What a button of the Recorder asks the app to do (taken once per frame).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsrRequest {
    /// Transcribe the last saved recording.
    Transcribe,
    /// Stop the running transcription.
    Cancel,
    /// Go to Settings > Transcription.
    OpenSettings,
}

/// Placeholder folder of the preview; replaced by a real one when the worker is connected.
pub const MOCK_DESTINATION: &str = "C:\\Users\\Nattapong\\Documents\\MeetingRecorder\\Recordings";

/// Level shown for a source that is not capturing.
const LEVEL_FLOOR_DB: f32 = -90.0;

/// How long the mock engine "loads" before becoming ready.
const ENGINE_LOAD_SECS: f64 = 1.5;

pub struct RecorderScreen {
    pub engine: EngineState,
    /// When true the engine goes Loading -> Ready by itself (off when forced by `--engine=`).
    pub engine_auto: bool,
    engine_started: Option<f64>,
    pub state: RecorderState,
    pub sys_audio_enabled: bool,
    pub mic_enabled: bool,
    pub sys_audio_level: f32,
    pub mic_level: f32,
    pub file_name: String,
    pub destination: String,
    pub transcript_search: String,
    pub auto_scroll: bool,
    /// The capture worker; `None` in preview mode (dev flags, tests), where the controls
    /// only change the on-screen state.
    pub audio: Option<AudioHandle>,
    /// Recorded time of the current or last session, from the worker.
    pub elapsed: Duration,
    /// The worker's last error message (shown in the error banner).
    pub last_error: Option<String>,
    /// Folder of the last saved recording.
    pub saved_dir: Option<PathBuf>,
    pub sys_status: Option<SourceStatus>,
    pub mic_status: Option<SourceStatus>,
    /// One-shot text for a toast (taken by the app once per frame).
    notice: Option<String>,
    /// Settings that go into the next `Start`; kept up to date by `sync_capture`.
    capture_rate: u32,
    capture_system: Option<String>,
    capture_mic: Option<String>,
    pub transcript_segments: Vec<TranscriptSegment>,
    pub partial_text: String,
    /// A live transcription run is on for the recording in progress.
    pub live_on: bool,
    /// The folder whose live lines the panel shows, and the start time of each of them (to put
    /// a late line in its place).
    live_for: Option<PathBuf>,
    live_starts: Vec<u64>,
    /// False in preview mode, where the panel shows the mock transcript.
    pub live: bool,
    pub asr_job: AsrJob,
    /// The recording folder whose job or transcript the panel shows.
    pub transcript_dir: Option<PathBuf>,
    /// Set by a button; taken by the app.
    pub request: Option<AsrRequest>,
}

impl RecorderScreen {
    /// Advances the mock engine load; returns `true` while it is still loading (keep repainting).
    pub fn tick_engine(&mut self, now: f64) -> bool {
        if self.engine_auto && self.engine == EngineState::Loading {
            let started = *self.engine_started.get_or_insert(now);
            if now - started >= ENGINE_LOAD_SECS {
                self.engine = EngineState::Ready;
            }
        }
        self.engine == EngineState::Loading && self.engine_auto
    }

    /// Start when idle / finished / failed; stop (finalize) when recording or paused.
    pub fn toggle_recording(&mut self) {
        match self.state {
            RecorderState::Recording | RecorderState::Paused => self.stop(),
            _ => self.start(),
        }
    }

    /// Pause <-> resume; ignored in other states. With a worker the state changes when the
    /// worker reports it.
    pub fn toggle_pause(&mut self) {
        let command = match self.state {
            RecorderState::Recording => Command::Pause,
            RecorderState::Paused => Command::Resume,
            _ => return,
        };
        match &self.audio {
            Some(audio) => audio.send(command),
            None => {
                self.state = if self.state == RecorderState::Recording {
                    RecorderState::Paused
                } else {
                    RecorderState::Recording
                }
            }
        }
    }

    /// Starts a session. Ignored while one is running or being finalized.
    pub fn start(&mut self) {
        if self.audio.is_none() {
            if !matches!(self.state, RecorderState::Recording | RecorderState::Paused) {
                self.state = RecorderState::Recording;
            }
            return;
        }
        if self.state.session_active() {
            return;
        }
        let request = self.start_request();
        self.last_error = None;
        self.saved_dir = None;
        self.sys_status = None;
        self.mic_status = None;
        self.elapsed = Duration::ZERO;
        self.state = RecorderState::Starting;
        if let Some(audio) = &self.audio {
            audio.send(Command::Start(request));
        }
    }

    /// Stops and finalizes a running or paused session.
    pub fn stop(&mut self) {
        if !matches!(self.state, RecorderState::Recording | RecorderState::Paused) {
            return;
        }
        match &self.audio {
            Some(audio) => audio.send(Command::Stop),
            None => self.state = RecorderState::Processing,
        }
    }

    /// Takes over the settings that the next `Start` needs.
    pub fn sync_capture(&mut self, sample_rate: u32, system: Option<String>, mic: Option<String>) {
        self.capture_rate = sample_rate;
        self.capture_system = system;
        self.capture_mic = mic;
    }

    pub fn start_request(&self) -> StartRequest {
        StartRequest {
            system: self.sys_audio_enabled,
            mic: self.mic_enabled,
            sample_rate: self.capture_rate,
            base_dir: PathBuf::from(self.destination.trim()),
            name: self.file_name.trim().to_owned(),
            system_device: self.capture_system.clone(),
            mic_device: self.capture_mic.clone(),
        }
    }

    /// Applies one report from the capture worker.
    pub fn apply_event(&mut self, event: Event) {
        match event {
            Event::State(state) => {
                self.state = match state {
                    SessionState::Idle => RecorderState::Idle,
                    SessionState::Starting => RecorderState::Starting,
                    SessionState::Recording => RecorderState::Recording,
                    SessionState::Paused => RecorderState::Paused,
                    SessionState::Stopping | SessionState::Finalizing => RecorderState::Processing,
                    SessionState::Saved => RecorderState::Saved,
                    SessionState::Error => RecorderState::Error,
                };
                if !self.state.session_active() {
                    self.sys_audio_level = LEVEL_FLOOR_DB;
                    self.mic_level = LEVEL_FLOOR_DB;
                }
            }
            Event::Levels { system_db, mic_db } => {
                self.sys_audio_level = system_db;
                self.mic_level = mic_db;
            }
            Event::SourceStatus { source, status } => match source {
                Source::System => self.sys_status = Some(status),
                Source::Mic => self.mic_status = Some(status),
            },
            Event::Elapsed(elapsed) => self.elapsed = elapsed,
            Event::SessionStarted { .. } => {} // the app starts the live transcript
            Event::Saved { dir, duration } => {
                self.elapsed = duration;
                let name = dir
                    .file_name()
                    .map_or_else(|| dir.to_string_lossy(), |n| n.to_string_lossy());
                self.notice = Some(format!("{} {}", tr("Recording saved to"), name));
                self.saved_dir = Some(dir);
            }
            Event::Error(message) => {
                // An error while Starting always ends the start (a source that cannot open is a
                // SourceStatus, not an Error), and a rejected start sends no State event.
                // Once a session runs, the worker's own State event says whether it survives.
                if matches!(
                    self.state,
                    RecorderState::Recording | RecorderState::Paused | RecorderState::Processing
                ) {
                    self.notice = Some(tr(&message).to_owned());
                } else {
                    self.state = RecorderState::Error;
                }
                self.last_error = Some(message);
            }
            Event::Devices { .. } => {} // the app routes device lists to the shared choice
        }
    }

    pub fn take_notice(&mut self) -> Option<String> {
        self.notice.take()
    }

    /// Big timer: the worker's elapsed time, or the fixed mock values in preview mode.
    pub fn timer_text(&self) -> String {
        if self.audio.is_some() {
            return clock(self.elapsed);
        }
        match self.state {
            RecorderState::Idle => "00:00:00".to_owned(),
            _ => "00:14:32".to_owned(),
        }
    }

    /// Text of the top-bar status chip.
    pub fn global_label(&self) -> String {
        if self.audio.is_none() {
            return match self.state {
                RecorderState::Idle => tr("Recorder Idle").to_owned(),
                RecorderState::Starting => tr("Starting").to_owned(),
                RecorderState::Recording => tr("Recording · 00:14:32").to_owned(),
                RecorderState::Paused => tr("Paused · 00:14:32").to_owned(),
                RecorderState::Processing => tr("Processing · Finalizing").to_owned(),
                RecorderState::Saved => tr("Saved").to_owned(),
                RecorderState::Error => tr("Error · Device lost").to_owned(),
            };
        }
        match self.state {
            RecorderState::Recording | RecorderState::Paused => {
                format!("{} · {}", self.state.label(), clock(self.elapsed))
            }
            RecorderState::Idle => tr("Recorder Idle").to_owned(),
            RecorderState::Processing => tr("Processing · Finalizing").to_owned(),
            other => other.label().to_owned(),
        }
    }

    /// "48 kHz · 2-ch" for the mock, the chosen rate in session mode.
    fn rate_label(&self) -> String {
        if self.audio.is_none() {
            return tr("48 kHz · 2-ch").to_owned();
        }
        let khz = f64::from(self.capture_rate) / 1000.0;
        format!("{khz} kHz · 2-ch")
    }

    /// Title and text of the error banner.
    fn banner_text(&self) -> (String, String) {
        match (&self.audio, &self.last_error) {
            (Some(_), Some(message)) => {
                (tr("Recording problem").to_owned(), tr(message).to_owned())
            }
            _ => (
                tr("WASAPI device disconnected").to_owned(),
                tr("Microphone input stream lost. Reconnect device and click Resume.").to_owned(),
            ),
        }
    }

    /// Text for the Copy button, with the speakers named in the UI language.
    pub fn transcript_text(&self) -> String {
        self.transcript_text_with(tr("You"), tr("Others"))
    }

    pub fn transcript_text_with(&self, you: &str, others: &str) -> String {
        self.transcript_segments
            .iter()
            .map(|s| match s.speaker {
                Some(Track::Mic) => format!("[{}] {you}: {}", s.timestamp, s.text),
                Some(Track::System) => format!("[{}] {others}: {}", s.timestamp, s.text),
                None => format!("[{}] {}", s.timestamp, s.text),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Leaves the preview: the mock transcript goes, real lines come.
    pub fn go_live(&mut self) {
        if !self.live {
            self.live = true;
            self.transcript_segments.clear();
            self.partial_text.clear();
        }
    }

    /// Whether events for `dir` may change the panel. A running job keeps the panel; once it is
    /// over, the next folder takes it.
    fn owns(&mut self, dir: &Path) -> bool {
        if self.transcript_dir.as_deref() == Some(dir) {
            return true;
        }
        if self.asr_job.is_running() {
            return false;
        }
        self.go_live();
        self.transcript_dir = Some(dir.to_path_buf());
        self.transcript_segments.clear();
        self.partial_text.clear();
        self.live_for = None;
        self.live_starts.clear();
        true
    }

    /// A live run starts for the recording in `dir`: the panel is emptied for it. `false` (and
    /// nothing changes) while a transcription job is on the panel.
    pub fn live_started(&mut self, dir: PathBuf) -> bool {
        if self.asr_job.is_running() {
            return false;
        }
        self.go_live();
        self.transcript_dir = Some(dir.clone());
        self.transcript_segments.clear();
        self.partial_text.clear();
        self.live_starts.clear();
        self.asr_job = AsrJob::Idle;
        self.live_for = Some(dir);
        self.live_on = true;
        true
    }

    /// A line recognized while recording; lines of another folder are ignored.
    pub fn live_line(&mut self, dir: &Path, line: &Line) {
        if self.live_for.as_deref() != Some(dir) {
            return;
        }
        let at = self.live_starts.partition_point(|&s| s <= line.start_ms);
        self.live_starts.insert(at, line.start_ms);
        self.transcript_segments.insert(
            at,
            TranscriptSegment {
                timestamp: format_clock(line.start_ms),
                text: line.text.clone(),
                speaker: Some(line.track),
            },
        );
    }

    /// The live run is over (the lines stay until the final transcript replaces them).
    pub fn live_stopped(&mut self) {
        self.live_on = false;
    }

    /// The worker says the live run for `dir` ended.
    pub fn live_ended(&mut self, dir: &Path) {
        if self.live_for.as_deref() == Some(dir) {
            self.live_on = false;
        }
    }

    /// A job for `dir` was sent to the worker.
    pub fn asr_started(&mut self, dir: PathBuf) {
        if self.asr_job.is_running() && self.transcript_dir.as_deref() == Some(dir.as_path()) {
            return;
        }
        if !self.owns(&dir) {
            return; // queued behind the job on screen
        }
        self.go_live();
        // The live lines of this recording stay until its transcript replaces them.
        if self.live_for.as_deref() != Some(dir.as_path()) {
            self.transcript_segments.clear();
            self.live_starts.clear();
        }
        self.partial_text.clear();
        self.live_on = false;
        self.asr_job = AsrJob::Running {
            done_secs: 0.0,
            total_secs: 0.0,
        };
    }

    /// The recording of `dir` is saved but cannot be transcribed yet.
    pub fn asr_waiting(&mut self, dir: PathBuf) {
        if self.owns(&dir) {
            self.asr_job = AsrJob::Waiting;
        }
    }

    pub fn asr_progress(&mut self, dir: &Path, done_secs: f32, total_secs: f32) {
        if self.owns(dir) {
            self.asr_job = AsrJob::Running {
                done_secs,
                total_secs,
            };
        }
    }

    pub fn asr_done(&mut self, dir: &Path, transcript: &Transcript) {
        if self.owns(dir) {
            self.set_transcript(transcript);
            self.asr_job = AsrJob::Done;
        }
    }

    pub fn asr_cancelled(&mut self, dir: &Path) {
        if self.owns(dir) {
            self.asr_job = AsrJob::Idle;
        }
    }

    pub fn asr_failed(&mut self, dir: &Path, message: String) {
        if self.owns(dir) {
            self.asr_job = AsrJob::Failed(message);
        }
    }

    /// Why the last transcription failed (its own banner; the recording is fine).
    pub fn asr_failure_text(&self) -> Option<&str> {
        match &self.asr_job {
            AsrJob::Failed(m) => Some(m),
            _ => None,
        }
    }

    pub fn take_request(&mut self) -> Option<AsrRequest> {
        self.request.take()
    }

    /// Shows the lines of a finished transcript.
    pub fn set_transcript(&mut self, t: &Transcript) {
        self.go_live();
        self.partial_text.clear();
        self.live_for = None;
        self.live_starts.clear();
        self.transcript_segments = t
            .lines
            .iter()
            .map(|l| TranscriptSegment {
                timestamp: format_clock(l.start_ms),
                text: l.text.clone(),
                speaker: Some(l.track),
            })
            .collect();
    }

    /// English key of the text shown while the panel has no lines.
    pub fn transcript_empty_state(&self) -> &'static str {
        if self.live_on && !self.asr_job.is_running() {
            return "Listening...";
        }
        match &self.asr_job {
            AsrJob::Running { .. } => "Transcribing...",
            AsrJob::Waiting => "Waiting for the model",
            AsrJob::Failed(_) => "Transcription failed",
            AsrJob::Done => "No speech found",
            AsrJob::Idle => "The transcript appears after you stop recording",
        }
    }

    /// Footer text of a running job.
    pub fn job_text(&self) -> String {
        match &self.asr_job {
            AsrJob::Running {
                done_secs,
                total_secs,
            } if *total_secs > 0.0 => format!(
                "{} {} / {}",
                tr("Transcribing"),
                clock(Duration::from_secs_f32(*done_secs)),
                clock(Duration::from_secs_f32(*total_secs))
            ),
            AsrJob::Running { .. } => tr("Transcribing...").to_owned(),
            _ => String::new(),
        }
    }
}

#[derive(Clone)]
pub struct TranscriptSegment {
    pub timestamp: String,
    pub text: String,
    /// Who spoke; `None` for the preview's mock lines.
    pub speaker: Option<Track>,
}

impl RecorderScreen {
    /// What the capture worker reported since the last frame (empty in preview mode).
    pub fn take_events(&self) -> Vec<Event> {
        self.audio
            .as_ref()
            .map(AudioHandle::try_events)
            .unwrap_or_default()
    }
}

impl Default for RecorderScreen {
    fn default() -> Self {
        Self {
            engine: EngineState::Loading,
            engine_auto: true,
            engine_started: None,
            state: RecorderState::Idle,
            sys_audio_enabled: true,
            mic_enabled: true,
            sys_audio_level: -14.0,
            mic_level: -19.0,
            file_name: "2026-11-23_q4-architecture-sync".to_owned(),
            destination: MOCK_DESTINATION.to_owned(),
            transcript_search: String::new(),
            auto_scroll: true,
            audio: None,
            elapsed: Duration::ZERO,
            last_error: None,
            saved_dir: None,
            sys_status: None,
            mic_status: None,
            notice: None,
            capture_rate: 48_000,
            capture_system: None,
            capture_mic: None,
            live_on: false,
            live_for: None,
            live_starts: Vec::new(),
            transcript_segments: vec![
                TranscriptSegment {
                    timestamp: "00:00:12".to_owned(),
                    speaker: None,
                    text: "สวัสดีครับทุกคน วันนี้เราจะมาคุยเกี่ยวกับความคืบหน้าของโปรเจกต์ระบบบันทึกการประชุมและการถอดเสียงภาษาไทยแบบเรียลไทม์".to_owned(),
                },
                TranscriptSegment {
                    timestamp: "00:02:45".to_owned(),
                    speaker: None,
                    text: "ตอนนี้ระบบหลักพัฒนาเสร็จแล้ว เหลือส่วนทดสอบและปรับปรุงประสิทธิภาพของตัวโมเดล OpenThai ASR บน Windows WASAPI".to_owned(),
                },
                TranscriptSegment {
                    timestamp: "00:05:18".to_owned(),
                    speaker: None,
                    text: "จากการทดสอบเมื่อวาน การบันทึกเสียงจากระบบและไมโครโฟนแยกแทร็กทำงานได้เสถียร ไม่มีอาการดีเลย์ระหว่างการประชุมยาวสองชั่วโมง".to_owned(),
                },
                TranscriptSegment {
                    timestamp: "00:08:54".to_owned(),
                    speaker: None,
                    text: "เรื่องการจัดการไฟล์เสียง เราจะบันทึกลงในเครื่องทั้งหมดก่อน แล้วเก็บดัชนีข้อความไว้ในฐานข้อมูล SQLite เพื่อให้ค้นหาคำภาษาไทยได้ทันที".to_owned(),
                },
                TranscriptSegment {
                    timestamp: "00:11:20".to_owned(),
                    speaker: None,
                    text: "สำหรับสัปดาห์หน้า ทีมจะเริ่มทดสอบการส่งออกไฟล์เป็น TXT, SRT และ JSON รวมถึงตรวจสอบการใช้หน่วยความจำของ ONNX Runtime".to_owned(),
                },
            ],
            partial_text: "เดี๋ยวเราจะลองเช็กค่าหน่วงเวลาของการสตรีมเสียงอีกครั้ง ก่อนที่จะปล่อยเวอร์ชันทดสอบภายใน...".to_owned(),
            live: false,
            asr_job: AsrJob::Idle,
            transcript_dir: None,
            request: None,
        }
    }
}

pub fn draw_recorder_screen(
    ui: &mut Ui,
    theme: &ThemeColors,
    screen: &mut RecorderScreen,
    choice: &mut AudioChoice,
) {
    // 35% / 65% split
    let left_width = (ui.available_width() * 0.35).max(320.0);

    egui::SidePanel::left("recorder_left_panel")
        .exact_width(left_width)
        .resizable(false)
        .frame(egui::Frame::NONE.fill(theme.background))
        .show_inside(ui, |ui| {
            draw_left_panel(ui, theme, screen, choice);
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(theme.background))
        .show_inside(ui, |ui| {
            // Vertical divider on the shared edge
            let rect = ui.max_rect();
            ui.painter().vline(
                rect.left(),
                rect.y_range(),
                egui::Stroke::new(1.0, theme.outline),
            );
            draw_right_panel(ui, theme, screen);
        });
}

fn draw_left_panel(
    ui: &mut Ui,
    theme: &ThemeColors,
    screen: &mut RecorderScreen,
    choice: &mut AudioChoice,
) {
    egui::ScrollArea::vertical()
        .id_salt("recorder_left")
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

            let pad = 24_i8;
            egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(pad, pad))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 16.0);

                    // ── Status & Timer Card ──────────────────────
                    draw_status_card(ui, theme, screen);

                    // ── Audio Sources Card ───────────────────────
                    draw_audio_sources_card(ui, theme, screen, choice);

                    // ── File Name & Storage Card ─────────────────
                    draw_file_storage_card(ui, theme, screen);
                });
        });
}

fn draw_status_card(ui: &mut Ui, theme: &ThemeColors, screen: &mut RecorderScreen) {
    egui::Frame::NONE
        .fill(theme.surface)
        .stroke(egui::Stroke::new(1.0, theme.outline))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 12.0);

            // Row: Session status + badge
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(tr("SESSION STATUS"))
                        .size(11.0)
                        .color(theme.on_surface_variant),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    draw_state_badge(ui, theme, screen.state);
                });
            });

            // Row: Timer + sample rate
            // `display_timer` (36px) needs ~290px next to the sample-rate label; below that use
            // `display_timer_compact` (28px) instead of overlapping.
            let timer_size = if ui.available_width() < 290.0 {
                28.0
            } else {
                36.0
            };
            ui.horizontal(|ui| {
                // Large elapsed timer
                ui.label(
                    RichText::new(screen.timer_text())
                        .font(egui::FontId::monospace(timer_size))
                        .family(crate::theme::mono_bold_family())
                        .color(theme.on_surface),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(screen.rate_label())
                            .font(egui::FontId::monospace(12.0))
                            .color(theme.on_surface_variant),
                    );
                });
            });

            // Error banner (only in error state)
            if screen.state == RecorderState::Error {
                let (banner_title, banner_body) = screen.banner_text();
                egui::Frame::NONE
                    .fill(Color32::from_rgba_unmultiplied(0xEF, 0x44, 0x44, 38))
                    .stroke(egui::Stroke::new(
                        1.0,
                        Color32::from_rgba_unmultiplied(0xEF, 0x44, 0x44, 102),
                    ))
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(egui_phosphor::regular::WARNING)
                                    .color(theme.error_text)
                                    .size(16.0),
                            );
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(banner_title)
                                        .size(11.0)
                                        .family(crate::theme::bold_family())
                                        .color(theme.on_surface),
                                );
                                ui.label(
                                    RichText::new(banner_body)
                                        .size(11.0)
                                        .color(theme.on_surface_variant),
                                );
                            });
                        });
                    });
            }

            // Saved folder after a finished session
            if screen.state == RecorderState::Saved {
                if let Some(dir) = &screen.saved_dir {
                    ui.label(
                        RichText::new(format!(
                            "{} {}",
                            tr("Recording saved to"),
                            dir.to_string_lossy()
                        ))
                        .size(11.0)
                        .color(theme.success_text),
                    );
                    if screen.live && !screen.asr_job.is_running() {
                        let resp = custom_button(
                            ui,
                            theme,
                            Some(egui_phosphor::regular::TEXT_ALIGN_LEFT),
                            tr("Transcribe"),
                            ButtonVariant::Secondary,
                        );
                        if resp.clicked() {
                            screen.request = Some(AsrRequest::Transcribe);
                        }
                    }
                }
            }

            // Transport controls 3-column grid
            let btn_w = ((ui.available_width() - 16.0) / 3.0).max(0.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);

                if block_button(
                    ui,
                    theme,
                    Some("●"),
                    tr("Record"),
                    ButtonVariant::Danger,
                    btn_w,
                )
                .clicked()
                {
                    screen.start();
                }
                let paused = screen.state == RecorderState::Paused;
                let (pause_icon, pause_label) = if paused {
                    (egui_phosphor::regular::PLAY, tr("Resume"))
                } else {
                    (egui_phosphor::regular::PAUSE, tr("Pause"))
                };
                if block_button(
                    ui,
                    theme,
                    Some(pause_icon),
                    pause_label,
                    ButtonVariant::Secondary,
                    btn_w,
                )
                .clicked()
                {
                    screen.toggle_pause();
                }
                if block_button(
                    ui,
                    theme,
                    Some(egui_phosphor::regular::STOP),
                    tr("Stop"),
                    ButtonVariant::Secondary,
                    btn_w,
                )
                .clicked()
                {
                    screen.stop();
                }
            });
        });
}

/// (fill, foreground, border) for a recorder-state badge, per design tokens.
pub fn state_badge_style(
    theme: &ThemeColors,
    state: RecorderState,
) -> (Color32, Color32, egui::Stroke) {
    match state {
        RecorderState::Idle => (
            theme.surface_elevated,
            theme.on_surface_variant,
            egui::Stroke::new(1.0, theme.outline),
        ),
        RecorderState::Recording | RecorderState::Error => {
            (theme.error, theme.on_error, egui::Stroke::NONE)
        }
        RecorderState::Paused => (theme.warning, theme.on_warning, egui::Stroke::NONE),
        RecorderState::Starting | RecorderState::Processing => (
            theme.primary_container,
            theme.on_primary_container,
            egui::Stroke::NONE,
        ),
        RecorderState::Saved => (
            theme.surface_elevated,
            theme.success_text,
            egui::Stroke::new(1.0, theme.outline),
        ),
    }
}

fn draw_state_badge(ui: &mut Ui, theme: &ThemeColors, state: RecorderState) {
    let (bg, fg, border) = state_badge_style(theme, state);
    egui::Frame::NONE
        .fill(bg)
        .stroke(border)
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(egui_phosphor::regular::RECORD)
                        .size(13.0)
                        .color(fg),
                );
                ui.label(RichText::new(state.label()).size(11.0).color(fg));
            });
        });
}

/// Small status text and colour next to a source name. `mock_error` is the preview mode's
/// "disconnected" look.
fn source_badge(
    theme: &ThemeColors,
    live: bool,
    status: Option<&SourceStatus>,
    mock_error: bool,
) -> (&'static str, Color32) {
    if !live {
        return if mock_error {
            (tr("Disconnected"), theme.error_text)
        } else {
            (tr("Connected"), theme.success_text)
        };
    }
    match status {
        None => (tr("Ready"), theme.on_surface_variant),
        Some(SourceStatus::Active) => (tr("Connected"), theme.success_text),
        Some(SourceStatus::AllSystemFallback) => (tr("All system audio"), theme.warning_text),
        Some(SourceStatus::WaitingForApp) => {
            (tr("Waiting for the meeting app"), theme.warning_text)
        }
        Some(SourceStatus::DeviceLost) => (tr("Device lost"), theme.error_text),
        Some(SourceStatus::Disabled) => (tr("Off"), theme.on_surface_variant),
        Some(SourceStatus::Failed(_)) => (tr("Could not start"), theme.error_text),
    }
}

fn draw_audio_sources_card(
    ui: &mut Ui,
    theme: &ThemeColors,
    screen: &mut RecorderScreen,
    choice: &mut AudioChoice,
) {
    let live = screen.audio.is_some();
    // The meters show the mock levels in preview mode, real ones only while capturing.
    let meters_on = !live
        || matches!(
            screen.state,
            RecorderState::Recording | RecorderState::Paused
        );
    egui::Frame::NONE
        .fill(theme.surface)
        .stroke(egui::Stroke::new(1.0, theme.outline))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 12.0);

            // Header row
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(tr("Audio sources"))
                        .size(14.0)
                        .family(crate::theme::bold_family())
                        .color(theme.on_surface),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(tr("WASAPI active"))
                            .size(11.0)
                            .color(theme.success_text),
                    );
                });
            });

            // ── System Audio ─────────────────────────────────
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "{}  {}",
                            egui_phosphor::regular::SPEAKER_HIGH,
                            tr("System Audio")
                        ))
                        .size(13.0)
                        .color(theme.on_surface),
                    );
                    let (text, color) =
                        source_badge(theme, live, screen.sys_status.as_ref(), false);
                    ui.label(RichText::new(text).size(11.0).color(color));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        toggle_switch(ui, theme, &mut screen.sys_audio_enabled);
                    });
                });

                let width = ui.available_width();
                let labels = choice.system_labels();
                if let Some(i) = device_dropdown(
                    ui,
                    theme,
                    "sys_device",
                    &labels,
                    choice.system_index(),
                    width,
                ) {
                    choice.select_system(i);
                }

                audio_level_meter(
                    ui,
                    theme,
                    "SYS",
                    screen.sys_audio_level,
                    screen.sys_audio_enabled && meters_on,
                );
            });

            // Thin separator
            ui.add(egui::Separator::default().horizontal().spacing(4.0));

            // ── Microphone ────────────────────────────────────
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);

                let (mic_status_text, mic_status_color) = source_badge(
                    theme,
                    live,
                    screen.mic_status.as_ref(),
                    screen.state == RecorderState::Error,
                );

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "{}  {}",
                            egui_phosphor::regular::MICROPHONE,
                            tr("Microphone")
                        ))
                        .size(13.0)
                        .color(theme.on_surface),
                    );
                    ui.label(
                        RichText::new(mic_status_text)
                            .size(11.0)
                            .color(mic_status_color),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        toggle_switch(ui, theme, &mut screen.mic_enabled);
                    });
                });

                let width = ui.available_width();
                let labels = choice.mic_labels();
                if let Some(i) =
                    device_dropdown(ui, theme, "mic_device", &labels, choice.mic_index(), width)
                {
                    choice.select_mic(i);
                }

                audio_level_meter(
                    ui,
                    theme,
                    "MIC",
                    screen.mic_level,
                    screen.mic_enabled && meters_on,
                );
            });
        });
}

fn draw_file_storage_card(ui: &mut Ui, theme: &ThemeColors, screen: &mut RecorderScreen) {
    egui::Frame::NONE
        .fill(theme.surface)
        .stroke(egui::Stroke::new(1.0, theme.outline))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 10.0);

            // File name
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 4.0);
                ui.label(
                    RichText::new(tr("Recording file name"))
                        .size(11.0)
                        .color(theme.on_surface_variant),
                );
                egui::Frame::NONE
                    .fill(theme.background)
                    .stroke(egui::Stroke::new(1.0, theme.outline))
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(10, 5))
                    .show(ui, |ui| {
                        ui.add_sized(
                            [ui.available_width(), 20.0],
                            egui::TextEdit::singleline(&mut screen.file_name)
                                .font(egui::FontId::monospace(12.0))
                                .text_color(theme.on_surface)
                                .frame(false),
                        );
                    });
            });

            // Destination
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 4.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(tr("Destination"))
                            .size(11.0)
                            .color(theme.on_surface_variant),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("184.2 GB {}", tr("free")))
                                .font(egui::FontId::monospace(12.0))
                                .color(theme.on_surface_variant),
                        );
                    });
                });

                egui::Frame::NONE
                    .fill(theme.background)
                    .stroke(egui::Stroke::new(1.0, theme.outline_variant))
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(10, 5))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(egui_phosphor::regular::FOLDER_OPEN)
                                    .size(12.0)
                                    .color(theme.on_surface_variant),
                            );
                            ui.add_sized(
                                [ui.available_width(), 20.0],
                                egui::Label::new(
                                    RichText::new(&screen.destination)
                                        .font(egui::FontId::monospace(11.0))
                                        .color(theme.on_surface_variant),
                                )
                                .truncate(),
                            );
                        });
                    });
            });
        });
}

fn draw_right_panel(ui: &mut Ui, theme: &ThemeColors, screen: &mut RecorderScreen) {
    // ── Header toolbar ─────────────────────────────────────────────────────
    egui::TopBottomPanel::top("transcript_header")
        .resizable(false)
        .frame(
            egui::Frame::NONE
                .fill(theme.surface)
                .stroke(egui::Stroke::new(1.0, theme.outline))
                .inner_margin(egui::Margin::symmetric(24, 12)),
        )
        .show_inside(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(12.0, 10.0);

            // Row 1: title + engine/language badges
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(if screen.live {
                        tr("Transcript")
                    } else {
                        tr("Live transcript")
                    })
                        .size(16.0)
                        .family(crate::theme::bold_family())
                        .color(theme.on_surface),
                );
                let (tint, text_color, icon, label) = match screen.engine {
                    EngineState::Ready => (
                        theme.success,
                        theme.success_text,
                        egui_phosphor::regular::DOT_OUTLINE,
                        tr("OpenThai ASR · ONNX Ready"),
                    ),
                    EngineState::Loading => (
                        theme.warning,
                        theme.warning_text,
                        egui_phosphor::regular::SPINNER_GAP,
                        tr("OpenThai ASR · Loading model..."),
                    ),
                    EngineState::Error => (
                        theme.error,
                        theme.error_text,
                        egui_phosphor::regular::WARNING,
                        tr("OpenThai ASR · Unavailable"),
                    ),
                    EngineState::Missing => (
                        theme.outline,
                        theme.on_surface_variant,
                        egui_phosphor::regular::DOWNLOAD_SIMPLE,
                        tr("OpenThai ASR · Model not installed"),
                    ),
                    EngineState::Downloading => (
                        theme.warning,
                        theme.warning_text,
                        egui_phosphor::regular::SPINNER_GAP,
                        tr("OpenThai ASR · Downloading model..."),
                    ),
                };
                tinted_badge(ui, tint, text_color, &format!("{icon} {label}"));
                tinted_badge(
                    ui,
                    theme.brand_blue,
                    theme.link_text,
                    &format!("{} {}", egui_phosphor::regular::TRANSLATE, tr("Thai (TH)")),
                );
            });

            if screen.engine == EngineState::Error {
                ui.label(
                    RichText::new(tr(
                        "Transcription engine unavailable. Audio is still recorded; transcribe it later from Recordings.",
                    ))
                    .size(12.0)
                    .color(theme.error_text),
                );
            }

            if let Some(message) = screen.asr_failure_text() {
                ui.label(
                    RichText::new(format!("{}: {}", tr("Transcription failed"), tr(message)))
                        .size(12.0)
                        .color(theme.error_text),
                );
            }
            if screen.asr_job == AsrJob::Waiting {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(tr("Waiting for the model"))
                            .size(12.0)
                            .color(theme.warning_text),
                    );
                    if custom_button(
                        ui,
                        theme,
                        None,
                        tr("Open Transcription settings"),
                        ButtonVariant::Secondary,
                    )
                    .clicked()
                    {
                        screen.request = Some(AsrRequest::OpenSettings);
                    }
                });
            }

            // Row 2: search, auto-scroll, copy
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                egui::Frame::NONE
                    .fill(theme.background)
                    .stroke(egui::Stroke::new(1.0, theme.outline))
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(10, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(egui_phosphor::regular::MAGNIFYING_GLASS)
                                    .size(14.0)
                                    .color(theme.on_surface_variant),
                            );
                            ui.add(
                                egui::TextEdit::singleline(&mut screen.transcript_search)
                                    .hint_text(tr("Search transcript..."))
                                    .desired_width(150.0)
                                    .text_color(theme.on_surface)
                                    .frame(false),
                            );
                        });
                    });

                egui::Frame::NONE
                    .fill(theme.background)
                    .stroke(egui::Stroke::new(1.0, theme.outline))
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(10, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(tr("Auto-scroll"))
                                    .size(12.0)
                                    .family(crate::theme::bold_family())
                                    .color(theme.on_surface),
                            );
                            toggle_switch(ui, theme, &mut screen.auto_scroll);
                        });
                    });

                let copy = ui
                    .add(
                        egui::Button::new(
                            RichText::new(format!(
                                "{}  {}",
                                egui_phosphor::regular::COPY,
                                tr("Copy")
                            ))
                            .size(12.0)
                            .family(crate::theme::bold_family())
                            .color(theme.on_surface),
                        )
                        .fill(theme.background)
                        .stroke(egui::Stroke::new(1.0, theme.outline))
                        .corner_radius(egui::CornerRadius::same(6))
                        .min_size(egui::vec2(0.0, 30.0)),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if copy.clicked() {
                    ui.ctx().copy_text(screen.transcript_text());
                }
            });
        });

    // ── Footer status strip ────────────────────────────────────────────────
    egui::TopBottomPanel::bottom("transcript_footer")
        .resizable(false)
        .frame(
            egui::Frame::NONE
                .fill(theme.surface)
                .stroke(egui::Stroke::new(1.0, theme.outline))
                .inner_margin(egui::Margin::symmetric(24, 8)),
        )
        .show_inside(ui, |ui| {
            if !screen.live {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(tr("5 finalized segments · 1 partial · 142 Thai words"))
                            .size(11.0)
                            .color(theme.on_surface_variant),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(tr("Latency: 210 ms · CPU: 14% (6 threads)"))
                                .font(egui::FontId::monospace(12.0))
                                .color(theme.on_surface_variant),
                        );
                    });
                });
            } else if screen.asr_job.is_running() {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(screen.job_text())
                            .font(egui::FontId::monospace(12.0))
                            .color(theme.on_surface_variant),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if custom_button(ui, theme, None, tr("Cancel"), ButtonVariant::Secondary)
                            .clicked()
                        {
                            screen.request = Some(AsrRequest::Cancel);
                        }
                    });
                });
            } else {
                let text = if screen.asr_job == AsrJob::Done {
                    format!("{} {}", screen.transcript_segments.len(), tr("lines"))
                } else {
                    String::new()
                };
                ui.label(
                    RichText::new(text)
                        .size(11.0)
                        .color(theme.on_surface_variant),
                );
            }
        });

    // ── Transcript stream ──────────────────────────────────────────────────
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(theme.surface))
        .show_inside(ui, |ui| {
            let query = screen.transcript_search.trim().to_lowercase();
            egui::ScrollArea::vertical()
                .id_salt("transcript_stream")
                .auto_shrink([false, false])
                .stick_to_bottom(screen.auto_scroll)
                .show(ui, |ui| {
                    egui::Frame::NONE
                        .inner_margin(egui::Margin::symmetric(24, 20))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.spacing_mut().item_spacing = egui::vec2(0.0, 18.0);

                            if screen.live && screen.transcript_segments.is_empty() {
                                ui.label(
                                    RichText::new(tr(screen.transcript_empty_state()))
                                        .size(14.0)
                                        .color(theme.on_surface_variant),
                                );
                            }
                            for seg in screen.transcript_segments.iter().filter(|s| {
                                query.is_empty() || s.text.to_lowercase().contains(&query)
                            }) {
                                ui.horizontal_top(|ui| {
                                    ui.spacing_mut().item_spacing.x = 16.0;
                                    ui.add_sized(
                                        [64.0, 22.0],
                                        egui::Label::new(
                                            RichText::new(&seg.timestamp)
                                                .font(egui::FontId::monospace(12.0))
                                                .color(theme.on_surface_variant),
                                        ),
                                    );
                                    if screen.live {
                                        ui.allocate_ui(egui::vec2(64.0, 22.0), |ui| {
                                            match seg.speaker {
                                                Some(Track::Mic) => tinted_badge(
                                                    ui,
                                                    theme.brand_blue,
                                                    theme.link_text,
                                                    tr("You"),
                                                ),
                                                Some(Track::System) => tinted_badge(
                                                    ui,
                                                    theme.outline,
                                                    theme.on_surface_variant,
                                                    tr("Others"),
                                                ),
                                                None => {}
                                            }
                                        });
                                    }
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(&seg.text)
                                                .size(15.0)
                                                .color(theme.on_surface),
                                        )
                                        .wrap(),
                                    );
                                });
                            }

                            // Partial (in-progress) segment: preview only
                            if !screen.live {
                                egui::Frame::NONE
                                    .fill(theme.background)
                                    .stroke(egui::Stroke::new(
                                        1.0,
                                        Color32::from_rgba_unmultiplied(0x3B, 0x82, 0xF6, 110),
                                    ))
                                    .corner_radius(egui::CornerRadius::same(8))
                                    .inner_margin(egui::Margin::same(12))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.horizontal_top(|ui| {
                                            ui.spacing_mut().item_spacing.x = 16.0;
                                            ui.vertical(|ui| {
                                                ui.spacing_mut().item_spacing.y = 4.0;
                                                ui.label(
                                                    RichText::new("00:14:28")
                                                        .font(egui::FontId::monospace(12.0))
                                                        .family(crate::theme::mono_bold_family())
                                                        .color(theme.on_primary_container),
                                                );
                                                egui::Frame::NONE
                                                    .fill(theme.primary_container)
                                                    .corner_radius(egui::CornerRadius::same(4))
                                                    .inner_margin(egui::Margin::symmetric(6, 2))
                                                    .show(ui, |ui| {
                                                        ui.label(
                                                            RichText::new(tr("Partial"))
                                                                .size(11.0)
                                                                .color(theme.on_primary_container),
                                                        );
                                                    });
                                            });
                                            ui.add(
                                                egui::Label::new(
                                                    RichText::new(&screen.partial_text)
                                                        .size(15.0)
                                                        .italics()
                                                        .color(theme.on_surface_variant),
                                                )
                                                .wrap(),
                                            );
                                        });
                                    });
                            }
                        });
                });
        });
}

/// Small tinted status pill (translucent fill + hairline border in `tint`).
/// Badge with a `tint` fill/border and a separately chosen, legible `text_color`.
pub(crate) fn tinted_badge(ui: &mut Ui, tint: Color32, text_color: Color32, text: &str) {
    egui::Frame::NONE
        .fill(Color32::from_rgba_unmultiplied(
            tint.r(),
            tint.g(),
            tint.b(),
            32,
        ))
        .stroke(egui::Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(tint.r(), tint.g(), tint.b(), 70),
        ))
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.label(
                RichText::new(text)
                    .size(11.0)
                    .family(crate::theme::bold_family())
                    .color(text_color),
            );
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::transcript::{Line, ModelInfo};

    #[test]
    fn toggle_recording_starts_and_stops() {
        let mut s = RecorderScreen::default();
        s.toggle_recording();
        assert_eq!(s.state, RecorderState::Recording);
        s.toggle_pause();
        assert_eq!(s.state, RecorderState::Paused);
        s.toggle_recording();
        assert_eq!(s.state, RecorderState::Processing);
        s.toggle_pause();
        assert_eq!(
            s.state,
            RecorderState::Processing,
            "pause is ignored while finalizing"
        );
    }

    use crate::audio::backend::FakeBackend;
    use crate::audio::session::{Event, SessionState, Source, SourceStatus};
    use crate::audio::worker::{AudioHandle, WorkerConfig};
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    /// A screen wired to a worker over a fake backend (session mode).
    fn live_screen() -> RecorderScreen {
        let handle = AudioHandle::spawn(Arc::new(FakeBackend::new()), WorkerConfig::default());
        let dir = crate::audio::temp_dir("rec-screen");
        RecorderScreen {
            audio: Some(handle),
            destination: dir.to_string_lossy().into_owned(),
            ..RecorderScreen::default()
        }
    }

    #[test]
    fn apply_state_events_map_to_recorder_state() {
        let mut s = RecorderScreen::default();
        for (from_worker, shown) in [
            (SessionState::Starting, RecorderState::Starting),
            (SessionState::Recording, RecorderState::Recording),
            (SessionState::Paused, RecorderState::Paused),
            (SessionState::Stopping, RecorderState::Processing),
            (SessionState::Finalizing, RecorderState::Processing),
            (SessionState::Saved, RecorderState::Saved),
            (SessionState::Error, RecorderState::Error),
            (SessionState::Idle, RecorderState::Idle),
        ] {
            s.apply_event(Event::State(from_worker));
            assert_eq!(s.state, shown, "worker state {from_worker:?}");
        }
    }

    #[test]
    fn apply_levels_updates_meters() {
        let mut s = RecorderScreen::default();
        s.apply_event(Event::Levels {
            system_db: -12.5,
            mic_db: -30.0,
        });
        assert_eq!(s.sys_audio_level, -12.5);
        assert_eq!(s.mic_level, -30.0);
    }

    #[test]
    fn apply_error_sets_banner_message_and_state() {
        let mut s = RecorderScreen::default();
        s.apply_event(Event::Error("disk full".into()));
        assert_eq!(s.state, RecorderState::Error);
        assert_eq!(s.last_error.as_deref(), Some("disk full"));
        // During a session the worker's own State(Error) follows; the state is not guessed.
        let mut s = RecorderScreen {
            state: RecorderState::Recording,
            ..RecorderScreen::default()
        };
        s.apply_event(Event::Error("write failed".into()));
        assert_eq!(s.state, RecorderState::Recording);
        s.apply_event(Event::State(SessionState::Error));
        assert_eq!(s.state, RecorderState::Error);
        assert_eq!(s.last_error.as_deref(), Some("write failed"));
    }

    #[test]
    fn apply_saved_records_folder_and_notice() {
        let mut s = RecorderScreen::default();
        let dir = PathBuf::from("D:\\Rec\\2026-11-24_0930_meeting");
        s.apply_event(Event::Saved {
            dir: dir.clone(),
            duration: Duration::from_secs(95),
        });
        assert_eq!(s.saved_dir, Some(dir));
        assert_eq!(s.elapsed, Duration::from_secs(95));
        let notice = s.take_notice().expect("a notice for the toast");
        assert!(notice.contains("2026-11-24_0930_meeting"), "{notice}");
        assert!(s.take_notice().is_none(), "the notice is taken once");
    }

    #[test]
    fn source_statuses_are_kept_per_source() {
        let mut s = RecorderScreen::default();
        s.apply_event(Event::SourceStatus {
            source: Source::System,
            status: SourceStatus::AllSystemFallback,
        });
        s.apply_event(Event::SourceStatus {
            source: Source::Mic,
            status: SourceStatus::Active,
        });
        assert_eq!(s.sys_status, Some(SourceStatus::AllSystemFallback));
        assert_eq!(s.mic_status, Some(SourceStatus::Active));
    }

    #[test]
    fn start_request_uses_screen_fields() {
        let mut s = RecorderScreen {
            sys_audio_enabled: false,
            mic_enabled: true,
            destination: "D:\\Rec".to_owned(),
            file_name: "Weekly sync".to_owned(),
            ..RecorderScreen::default()
        };
        s.sync_capture(44_100, Some("spk-1".to_owned()), None);
        let r = s.start_request();
        assert!(!r.system && r.mic);
        assert_eq!(r.sample_rate, 44_100);
        assert_eq!(r.base_dir, PathBuf::from("D:\\Rec"));
        assert_eq!(r.name, "Weekly sync");
        assert_eq!(r.system_device.as_deref(), Some("spk-1"));
        assert_eq!(r.mic_device, None);
        s.file_name.clear();
        assert_eq!(
            s.start_request().name,
            "",
            "an empty name is fixed by the worker's naming"
        );
    }

    #[test]
    fn timer_text_formats_elapsed() {
        let mut s = live_screen();
        s.elapsed = Duration::from_secs(3725);
        assert_eq!(s.timer_text(), "01:02:05");
        s.elapsed = Duration::ZERO;
        assert_eq!(s.timer_text(), "00:00:00");
    }

    #[test]
    fn preview_mode_keeps_the_mock_timer() {
        let mut s = RecorderScreen::default();
        assert_eq!(s.timer_text(), "00:00:00");
        s.toggle_recording();
        assert_eq!(s.timer_text(), "00:14:32");
    }

    #[test]
    fn session_global_label_shows_elapsed() {
        let mut s = live_screen();
        s.state = RecorderState::Recording;
        s.elapsed = Duration::from_secs(61);
        assert!(
            s.global_label().ends_with("00:01:01"),
            "{}",
            s.global_label()
        );
    }

    #[test]
    fn start_with_both_sources_off_ends_in_error_not_starting() {
        let mut s = live_screen();
        s.sys_audio_enabled = false;
        s.mic_enabled = false;
        s.start();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while s.state == RecorderState::Starting && std::time::Instant::now() < deadline {
            for e in s.take_events() {
                s.apply_event(e);
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(
            s.state,
            RecorderState::Error,
            "the worker rejects the start"
        );
        assert_eq!(
            s.last_error.as_deref(),
            Some("Turn on at least one audio source")
        );
        // The screen must be usable again: a new Start goes out.
        s.sys_audio_enabled = true;
        s.start();
        assert_eq!(s.state, RecorderState::Starting);
    }

    #[test]
    fn start_is_ignored_while_a_session_is_active() {
        let mut s = live_screen();
        s.start();
        assert_eq!(s.state, RecorderState::Starting);
        s.start(); // a second click must not send a second Start
        assert_eq!(s.state, RecorderState::Starting);
        s.state = RecorderState::Processing;
        s.start();
        assert_eq!(s.state, RecorderState::Processing);
    }

    #[test]
    fn new_status_labels_have_thai() {
        for key in [
            "Starting",
            "Saved",
            "Resume",
            "Ready",
            "Off",
            "All system audio",
            "Waiting for the meeting app",
            "Device lost",
            "Could not start",
            "Recording problem",
            "Recording saved to",
            "Turn on at least one audio source",
            "No audio source could be started",
            "A recording is already running",
        ] {
            assert!(crate::i18n::has_thai(key), "missing Thai for {key:?}");
        }
    }

    #[test]
    fn engine_loads_then_becomes_ready() {
        let mut s = RecorderScreen::default();
        assert!(s.tick_engine(10.0), "still loading on first tick");
        assert_eq!(s.engine, EngineState::Loading);
        assert!(!s.tick_engine(10.0 + ENGINE_LOAD_SECS));
        assert_eq!(s.engine, EngineState::Ready);
    }

    #[test]
    fn forced_engine_state_is_not_advanced() {
        let mut s = RecorderScreen {
            engine: EngineState::Loading,
            engine_auto: false,
            ..RecorderScreen::default()
        };
        assert!(!s.tick_engine(99.0));
        assert_eq!(s.engine, EngineState::Loading);
    }

    // ---- transcription of saved recordings ---------------------------------------------

    fn line(track: Track, start_ms: u64, text: &str) -> Line {
        Line {
            track,
            start_ms,
            end_ms: start_ms + 1000,
            text: text.to_owned(),
        }
    }

    fn transcript(lines: Vec<Line>) -> Transcript {
        Transcript {
            model: ModelInfo {
                name: "m".to_owned(),
                revision: "r".to_owned(),
                precision: "int8".to_owned(),
            },
            created: "2026-11-24T10:00:00".to_owned(),
            lines,
        }
    }

    #[test]
    fn real_lines_replace_the_mock_and_keep_their_speaker() {
        let mut s = RecorderScreen::default();
        assert_eq!(s.transcript_segments.len(), 5, "preview has mock lines");
        assert!(!s.live);
        s.set_transcript(&transcript(vec![
            line(Track::Mic, 3_000, "สวัสดี"),
            line(Track::System, 65_000, "ครับ"),
        ]));
        assert!(s.live);
        assert_eq!(s.transcript_segments.len(), 2);
        assert_eq!(s.transcript_segments[0].timestamp, "00:00:03");
        assert_eq!(s.transcript_segments[0].speaker, Some(Track::Mic));
        assert_eq!(s.transcript_segments[1].timestamp, "00:01:05");
        assert_eq!(s.transcript_segments[1].speaker, Some(Track::System));
        assert!(s.partial_text.is_empty(), "no mock partial line");
    }

    #[test]
    fn copied_text_names_the_speakers() {
        let mut s = RecorderScreen::default();
        s.set_transcript(&transcript(vec![
            line(Track::Mic, 0, "a"),
            line(Track::System, 2_000, "b"),
        ]));
        assert_eq!(
            s.transcript_text_with("You", "Others"),
            "[00:00:00] You: a\n[00:00:02] Others: b"
        );
    }

    #[test]
    fn progress_updates_the_running_job() {
        let mut s = RecorderScreen::default();
        let dir = PathBuf::from("rec1");
        s.asr_started(dir.clone());
        assert_eq!(
            s.asr_job,
            AsrJob::Running {
                done_secs: 0.0,
                total_secs: 0.0
            }
        );
        s.asr_progress(&dir, 5.0, 20.0);
        assert_eq!(
            s.asr_job,
            AsrJob::Running {
                done_secs: 5.0,
                total_secs: 20.0
            }
        );
        assert_eq!(s.transcript_dir.as_deref(), Some(dir.as_path()));
    }

    #[test]
    fn a_second_folder_waits_until_the_first_job_is_done() {
        let mut s = RecorderScreen::default();
        let (a, b) = (PathBuf::from("a"), PathBuf::from("b"));
        s.asr_started(a.clone());
        s.asr_started(b.clone()); // queued behind a: the panel stays on a
        assert_eq!(s.transcript_dir.as_deref(), Some(a.as_path()));
        s.asr_progress(&b, 1.0, 2.0); // not yet: a still runs
        assert_eq!(s.transcript_dir.as_deref(), Some(a.as_path()));

        s.asr_done(&a, &transcript(vec![line(Track::Mic, 0, "first")]));
        assert_eq!(s.asr_job, AsrJob::Done);
        assert_eq!(s.transcript_segments[0].text, "first");

        s.asr_progress(&b, 0.0, 2.0); // b starts: the panel moves to it
        assert_eq!(s.transcript_dir.as_deref(), Some(b.as_path()));
        assert!(matches!(s.asr_job, AsrJob::Running { .. }));
        assert!(s.transcript_segments.is_empty(), "a's lines are gone");
        s.asr_done(&b, &transcript(vec![line(Track::System, 0, "second")]));
        assert_eq!(s.transcript_segments[0].text, "second");
    }

    #[test]
    fn cancel_resets_the_job_and_error_shows_its_message() {
        let mut s = RecorderScreen::default();
        let dir = PathBuf::from("rec1");
        s.asr_started(dir.clone());
        s.asr_cancelled(&dir);
        assert_eq!(s.asr_job, AsrJob::Idle);
        assert!(s.asr_failure_text().is_none());

        s.asr_started(dir.clone());
        s.asr_failed(&dir, "Cannot read mic.wav: bad".to_owned());
        assert_eq!(
            s.asr_job,
            AsrJob::Failed("Cannot read mic.wav: bad".to_owned())
        );
        assert_eq!(s.asr_failure_text(), Some("Cannot read mic.wav: bad"));
        assert_ne!(
            s.state,
            RecorderState::Error,
            "the recording itself is fine"
        );
        assert!(s.last_error.is_none(), "not the recorder's red banner");
    }

    #[test]
    fn a_running_job_does_not_block_a_new_recording() {
        let mut s = RecorderScreen::default();
        s.asr_started(PathBuf::from("rec1"));
        s.start();
        assert_eq!(s.state, RecorderState::Recording);
        assert!(matches!(s.asr_job, AsrJob::Running { .. }));
    }

    #[test]
    fn empty_state_says_what_to_wait_for() {
        let mut s = RecorderScreen::default();
        s.go_live();
        assert_eq!(
            s.transcript_empty_state(),
            "The transcript appears after you stop recording"
        );
        s.asr_started(PathBuf::from("r"));
        assert_eq!(s.transcript_empty_state(), "Transcribing...");
        s.asr_done(&PathBuf::from("r"), &transcript(vec![]));
        assert_eq!(s.transcript_empty_state(), "No speech found");
        s.asr_waiting(PathBuf::from("r2"));
        assert_eq!(s.transcript_empty_state(), "Waiting for the model");
        s.asr_failed(&PathBuf::from("r2"), "x".to_owned());
        assert_eq!(s.transcript_empty_state(), "Transcription failed");
    }

    #[test]
    fn buttons_leave_a_request_for_the_app() {
        let mut s = RecorderScreen::default();
        assert_eq!(s.take_request(), None);
        s.request = Some(AsrRequest::Cancel);
        assert_eq!(s.take_request(), Some(AsrRequest::Cancel));
        assert_eq!(s.take_request(), None, "taken once");
    }

    #[test]
    fn progress_text_shows_clock_times() {
        let mut s = RecorderScreen::default();
        s.asr_started(PathBuf::from("r"));
        assert!(!s.job_text().is_empty());
        s.asr_progress(&PathBuf::from("r"), 23.0, 65.0);
        // The words follow the UI language, the times do not.
        assert!(
            s.job_text().ends_with("00:00:23 / 00:01:05"),
            "{}",
            s.job_text()
        );
    }

    #[test]
    fn new_recorder_texts_have_thai() {
        for key in [
            "Transcript",
            "The transcript appears after you stop recording",
            "No speech found",
            "Transcribing...",
            "Transcribing",
            "Waiting for the model",
            "Waiting for the model. The recording is saved; transcribe it after the model is downloaded.",
            "Transcription failed",
            "Transcription cancelled",
            "Open Transcription settings",
            "You",
            "Others",
            "lines",
        ] {
            assert!(crate::i18n::has_thai(key), "missing Thai for {key:?}");
        }
    }

    // ─── live lines ──────────────────────────────────────────────────────────────────────────

    fn live(track: Track, start_ms: u64, text: &str) -> Line {
        Line {
            track,
            start_ms,
            end_ms: start_ms + 1_000,
            text: text.to_owned(),
        }
    }

    #[test]
    fn live_lines_are_inserted_in_time_order_with_their_label() {
        let dir = PathBuf::from("C:\\rec\\a");
        let mut s = RecorderScreen::default();
        assert!(s.live_started(dir.clone()));
        s.live_line(&dir, &live(Track::Mic, 5_000, "b"));
        s.live_line(&dir, &live(Track::System, 1_000, "a"));
        s.live_line(&dir, &live(Track::Mic, 9_000, "c"));
        let texts: Vec<_> = s
            .transcript_segments
            .iter()
            .map(|x| x.text.as_str())
            .collect();
        assert_eq!(texts, ["a", "b", "c"]);
        let who: Vec<_> = s.transcript_segments.iter().map(|x| x.speaker).collect();
        assert_eq!(
            who,
            [Some(Track::System), Some(Track::Mic), Some(Track::Mic)]
        );
        assert_eq!(s.transcript_segments[0].timestamp, format_clock(1_000));
        // A line for another folder is ignored.
        s.live_line(Path::new("C:\\rec\\other"), &live(Track::Mic, 0, "x"));
        assert_eq!(s.transcript_segments.len(), 3);
    }

    #[test]
    fn a_new_live_run_clears_the_old_lines() {
        let a = PathBuf::from("C:\\rec\\a");
        let b = PathBuf::from("C:\\rec\\b");
        let mut s = RecorderScreen::default();
        assert!(s.live_started(a.clone()));
        s.live_line(&a, &live(Track::Mic, 0, "old"));
        assert!(s.live_started(b.clone()));
        assert!(s.transcript_segments.is_empty());
        assert_eq!(s.transcript_dir.as_deref(), Some(b.as_path()));
        s.live_line(&a, &live(Track::Mic, 0, "late line of the old run"));
        assert!(
            s.transcript_segments.is_empty(),
            "the old folder is not shown any more"
        );
    }

    #[test]
    fn live_does_not_take_the_panel_from_a_running_job() {
        let old = PathBuf::from("C:\\rec\\old");
        let new = PathBuf::from("C:\\rec\\new");
        let mut s = RecorderScreen::default();
        s.asr_started(old.clone());
        assert!(!s.live_started(new.clone()), "a job is running");
        assert_eq!(s.transcript_dir.as_deref(), Some(old.as_path()));
        assert!(!s.live_on);
    }

    #[test]
    fn listening_shows_until_the_run_ends_and_the_final_transcript_replaces_live_lines() {
        let dir = PathBuf::from("C:\\rec\\a");
        let mut s = RecorderScreen::default();
        assert!(s.live_started(dir.clone()));
        assert_eq!(s.transcript_empty_state(), "Listening...");
        s.live_line(&dir, &live(Track::Mic, 0, "live"));
        s.live_stopped();
        assert!(!s.live_on);

        // The final job starts: the live lines stay until its transcript arrives.
        s.asr_started(dir.clone());
        assert_eq!(
            s.transcript_segments.len(),
            1,
            "live lines are kept while it runs"
        );
        let t = Transcript {
            model: ModelInfo {
                name: "m".into(),
                revision: "r".into(),
                precision: "int8".into(),
            },
            created: "2026-11-25T10:00:00".into(),
            lines: vec![
                live(Track::Mic, 0, "final one"),
                live(Track::System, 2_000, "final two"),
            ],
        };
        s.asr_done(&dir, &t);
        let texts: Vec<_> = s
            .transcript_segments
            .iter()
            .map(|x| x.text.as_str())
            .collect();
        assert_eq!(texts, ["final one", "final two"]);
    }
}
