use crate::i18n::tr;
use egui::{Color32, RichText, Ui};

use crate::components::audio_meter::audio_level_meter;
use crate::components::button::{block_button, ButtonVariant};
use crate::components::toggle::toggle_switch;
use crate::theme::ThemeColors;

use crate::audio::session::{Command, Event, SessionState, Source, SourceStatus, StartRequest};
use crate::audio::worker::AudioHandle;
use crate::screens::settings::audio::{device_dropdown, AudioChoice};
use std::path::PathBuf;
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
    fn session_active(self) -> bool {
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

/// Transcription engine readiness (mock until a real engine exists).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EngineState {
    Loading,
    #[default]
    Ready,
    Error,
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
            Event::Saved { dir, duration } => {
                self.elapsed = duration;
                let name = dir
                    .file_name()
                    .map_or_else(|| dir.to_string_lossy(), |n| n.to_string_lossy());
                self.notice = Some(format!("{} {}", tr("Recording saved to"), name));
                self.saved_dir = Some(dir);
            }
            Event::Error(message) => {
                if self.state.session_active() {
                    // The worker's own State event says whether the session survives.
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

    pub fn transcript_text(&self) -> String {
        self.transcript_segments
            .iter()
            .map(|s| format!("[{}] {}", s.timestamp, s.text))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Clone)]
pub struct TranscriptSegment {
    pub timestamp: String,
    pub text: String,
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
            transcript_segments: vec![
                TranscriptSegment {
                    timestamp: "00:00:12".to_owned(),
                    text: "สวัสดีครับทุกคน วันนี้เราจะมาคุยเกี่ยวกับความคืบหน้าของโปรเจกต์ระบบบันทึกการประชุมและการถอดเสียงภาษาไทยแบบเรียลไทม์".to_owned(),
                },
                TranscriptSegment {
                    timestamp: "00:02:45".to_owned(),
                    text: "ตอนนี้ระบบหลักพัฒนาเสร็จแล้ว เหลือส่วนทดสอบและปรับปรุงประสิทธิภาพของตัวโมเดล OpenThai ASR บน Windows WASAPI".to_owned(),
                },
                TranscriptSegment {
                    timestamp: "00:05:18".to_owned(),
                    text: "จากการทดสอบเมื่อวาน การบันทึกเสียงจากระบบและไมโครโฟนแยกแทร็กทำงานได้เสถียร ไม่มีอาการดีเลย์ระหว่างการประชุมยาวสองชั่วโมง".to_owned(),
                },
                TranscriptSegment {
                    timestamp: "00:08:54".to_owned(),
                    text: "เรื่องการจัดการไฟล์เสียง เราจะบันทึกลงในเครื่องทั้งหมดก่อน แล้วเก็บดัชนีข้อความไว้ในฐานข้อมูล SQLite เพื่อให้ค้นหาคำภาษาไทยได้ทันที".to_owned(),
                },
                TranscriptSegment {
                    timestamp: "00:11:20".to_owned(),
                    text: "สำหรับสัปดาห์หน้า ทีมจะเริ่มทดสอบการส่งออกไฟล์เป็น TXT, SRT และ JSON รวมถึงตรวจสอบการใช้หน่วยความจำของ ONNX Runtime".to_owned(),
                },
            ],
            partial_text: "เดี๋ยวเราจะลองเช็กค่าหน่วงเวลาของการสตรีมเสียงอีกครั้ง ก่อนที่จะปล่อยเวอร์ชันทดสอบภายใน...".to_owned(),
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
                    RichText::new(tr("Live transcript"))
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

                            // Partial (in-progress) segment
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
                        });
                });
        });
}

/// Small tinted status pill (translucent fill + hairline border in `tint`).
/// Badge with a `tint` fill/border and a separately chosen, legible `text_color`.
fn tinted_badge(ui: &mut Ui, tint: Color32, text_color: Color32, text: &str) {
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
}
