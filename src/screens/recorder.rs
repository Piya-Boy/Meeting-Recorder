use crate::i18n::tr;
use egui::{Color32, RichText, Ui};

use crate::components::audio_meter::audio_level_meter;
use crate::components::button::{block_button, ButtonVariant};
use crate::components::select::dropdown;
use crate::components::toggle::toggle_switch;
use crate::theme::ThemeColors;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RecorderState {
    #[default]
    Idle,
    Recording,
    Paused,
    Processing,
    Error,
}

impl RecorderState {
    pub fn label(self) -> &'static str {
        match self {
            RecorderState::Idle => tr("Idle"),
            RecorderState::Recording => tr("Recording"),
            RecorderState::Paused => tr("Paused"),
            RecorderState::Processing => tr("Processing"),
            RecorderState::Error => tr("Error"),
        }
    }

    pub fn global_label(self) -> &'static str {
        match self {
            RecorderState::Idle => tr("Recorder Idle"),
            RecorderState::Recording => tr("Recording · 00:14:32"),
            RecorderState::Paused => tr("Paused · 00:14:32"),
            RecorderState::Processing => tr("Processing · Finalizing"),
            RecorderState::Error => tr("Error · Device lost"),
        }
    }

    pub fn timer(self) -> &'static str {
        match self {
            RecorderState::Idle => "00:00:00",
            _ => "00:14:32",
        }
    }
}

/// Transcription engine readiness (mock until a real engine exists).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EngineState {
    Loading,
    #[default]
    Ready,
    Error,
}

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
    pub sys_device_idx: usize,
    pub mic_device_idx: usize,
    pub file_name: String,
    pub destination: String,
    pub transcript_search: String,
    pub auto_scroll: bool,
    pub sys_devices: Vec<String>,
    pub mic_devices: Vec<String>,
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
        self.state = match self.state {
            RecorderState::Recording | RecorderState::Paused => RecorderState::Processing,
            _ => RecorderState::Recording,
        };
    }

    /// Pause <-> resume; ignored in other states.
    pub fn toggle_pause(&mut self) {
        match self.state {
            RecorderState::Recording => self.state = RecorderState::Paused,
            RecorderState::Paused => self.state = RecorderState::Recording,
            _ => {}
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
            sys_device_idx: 0,
            mic_device_idx: 0,
            file_name: "2026-11-23_q4-architecture-sync".to_owned(),
            destination: "C:\\Users\\Nattapong\\Documents\\MeetingRecorder\\Recordings".to_owned(),
            transcript_search: String::new(),
            auto_scroll: true,
            sys_devices: vec![
                "Speakers (Realtek(R) Audio) — Loopback".to_owned(),
                "Headphones (Sony WH-1000XM5)".to_owned(),
                "DELL U2723QE (NVIDIA HD Audio)".to_owned(),
            ],
            mic_devices: vec![
                "Shure MV7 USB Podcast Microphone".to_owned(),
                "Microphone Array (Intel Smart Sound)".to_owned(),
                "Headset (Sony WH-1000XM5 Hands-Free)".to_owned(),
            ],
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

pub fn draw_recorder_screen(ui: &mut Ui, theme: &ThemeColors, screen: &mut RecorderScreen) {
    // 35% / 65% split
    let left_width = (ui.available_width() * 0.35).max(320.0);

    egui::SidePanel::left("recorder_left_panel")
        .exact_width(left_width)
        .resizable(false)
        .frame(egui::Frame::NONE.fill(theme.background))
        .show_inside(ui, |ui| {
            draw_left_panel(ui, theme, screen);
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

fn draw_left_panel(ui: &mut Ui, theme: &ThemeColors, screen: &mut RecorderScreen) {
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
                    draw_audio_sources_card(ui, theme, screen);

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
            let timer_size = if ui.available_width() < 290.0 { 28.0 } else { 36.0 };
            ui.horizontal(|ui| {
                // Large elapsed timer
                ui.label(
                    RichText::new(screen.state.timer())
                        .font(egui::FontId::monospace(timer_size))
                        .family(crate::theme::mono_bold_family())
                        .color(theme.on_surface),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(tr("48 kHz · 2-ch"))
                            .font(egui::FontId::monospace(12.0))
                            .color(theme.on_surface_variant),
                    );
                });
            });

            // Error banner (only in error state)
            if screen.state == RecorderState::Error {
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
                            ui.label(RichText::new(egui_phosphor::regular::WARNING).color(theme.error_text).size(16.0));
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(tr("WASAPI device disconnected"))
                                        .size(11.0)
                                        .family(crate::theme::bold_family())
                                        .color(theme.on_surface),
                                );
                                ui.label(
                                    RichText::new(tr("Microphone input stream lost. Reconnect device and click Resume."))
                                        .size(11.0)
                                        .color(theme.on_surface_variant),
                                );
                            });
                        });
                    });
            }

            // Transport controls 3-column grid
            let btn_w = ((ui.available_width() - 16.0) / 3.0).max(0.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);

                // Record (error red), Pause / Stop (secondary): 3-column grid.
                let transport = [
                    ("●", tr("Record"), ButtonVariant::Danger, RecorderState::Recording),
                    (
                        egui_phosphor::regular::PAUSE,
                        tr("Pause"),
                        ButtonVariant::Secondary,
                        RecorderState::Paused,
                    ),
                    (
                        egui_phosphor::regular::STOP,
                        tr("Stop"),
                        ButtonVariant::Secondary,
                        RecorderState::Processing,
                    ),
                ];
                for (icon, label, variant, next) in transport {
                    if block_button(ui, theme, Some(icon), label, variant, btn_w).clicked() {
                        screen.state = next;
                    }
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
        RecorderState::Processing => (
            theme.primary_container,
            theme.on_primary_container,
            egui::Stroke::NONE,
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

fn draw_audio_sources_card(ui: &mut Ui, theme: &ThemeColors, screen: &mut RecorderScreen) {
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
                    ui.label(
                        RichText::new(tr("Connected"))
                            .size(11.0)
                            .color(theme.success_text),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        toggle_switch(ui, theme, &mut screen.sys_audio_enabled);
                    });
                });

                dropdown(
                    ui,
                    theme,
                    "sys_device",
                    &mut screen.sys_device_idx,
                    &screen.sys_devices,
                    ui.available_width(),
                    theme.background,
                    false,
                );

                audio_level_meter(
                    ui,
                    theme,
                    "SYS",
                    screen.sys_audio_level,
                    screen.sys_audio_enabled,
                );
            });

            // Thin separator
            ui.add(egui::Separator::default().horizontal().spacing(4.0));

            // ── Microphone ────────────────────────────────────
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);

                let mic_status_color = if screen.state == RecorderState::Error {
                    theme.error_text
                } else {
                    theme.success_text
                };
                let mic_status_text = if screen.state == RecorderState::Error {
                    tr("Disconnected")
                } else {
                    tr("Connected")
                };

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

                dropdown(
                    ui,
                    theme,
                    "mic_device",
                    &mut screen.mic_device_idx,
                    &screen.mic_devices,
                    ui.available_width(),
                    theme.background,
                    false,
                );

                audio_level_meter(ui, theme, "MIC", screen.mic_level, screen.mic_enabled);
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
