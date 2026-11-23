use crate::i18n::tr;
use egui::{RichText, Ui};

use crate::app::Screen;
use crate::components::button::{compact_button, icon_button, mono_chip_button, ButtonVariant};
use crate::components::input::search_box;
use crate::components::table::{body_row, header_row, table_frame, Align, Column};
use crate::theme::ThemeColors;
use crate::toast;

#[derive(Clone, Debug)]
pub struct TranscriptEntry {
    pub title: String,
    pub date: String,
    pub duration: String,
    pub words: String,
    pub preview: String,
}

pub struct TranscriptsScreen {
    pub search: String,
    pub transcripts: Vec<TranscriptEntry>,
}

impl Default for TranscriptsScreen {
    fn default() -> Self {
        Self {
            search: String::new(),
            transcripts: vec![
                TranscriptEntry {
                    title: "Q4 Engineering Architecture Sync".to_owned(),
                    date: "2026-11-23 09:30".to_owned(),
                    duration: "00:42:18".to_owned(),
                    words: "3,420".to_owned(),
                    preview: "วันนี้เราจะมาคุยเกี่ยวกับความคืบหน้าของโปรเจกต์ระบบบันทึกการประชุม...".to_owned(),
                },
                TranscriptEntry {
                    title: "Bangkok Client Onboarding Review".to_owned(),
                    date: "2026-11-22 14:00".to_owned(),
                    duration: "01:08:44".to_owned(),
                    words: "5,890".to_owned(),
                    preview: "ตอนนี้ระบบหลักพัฒนาเสร็จแล้ว เหลือส่วนทดสอบและปรับปรุงประสิทธิภาพ...".to_owned(),
                },
                TranscriptEntry {
                    title: "Sprint 18 Retrospective".to_owned(),
                    date: "2026-11-18 15:30".to_owned(),
                    duration: "00:28:04".to_owned(),
                    words: "2,180".to_owned(),
                    preview: "สรุปงานในรอบสองสัปดาห์ที่ผ่านมา ปัญหาเรื่องการเชื่อมต่อเสียงและแนวทางแก้ไข..."
                        .to_owned(),
                },
                TranscriptEntry {
                    title: "WASAPI Integration Design Review".to_owned(),
                    date: "2026-11-15 10:00".to_owned(),
                    duration: "00:55:12".to_owned(),
                    words: "4,310".to_owned(),
                    preview: "การออกแบบ API สำหรับ Windows Audio Session API และการจัดการ Buffer..."
                        .to_owned(),
                },
            ],
        }
    }
}

const COLS: [Column; 5] = [
    Column::flex(),
    Column::fixed(96.0, Align::Left),
    Column::fixed(76.0, Align::Right),
    Column::fixed(64.0, Align::Right),
    Column::fixed(236.0, Align::Right),
];

/// Draws the saved-transcripts table. Returns a screen to navigate to, if any.
pub fn draw_transcripts_screen(
    ui: &mut Ui,
    theme: &ThemeColors,
    screen: &mut TranscriptsScreen,
) -> Option<Screen> {
    let mut navigate = None;
    egui::CentralPanel::default()
        .frame(
            egui::Frame::NONE
                .fill(theme.background)
                .inner_margin(egui::Margin::same(24)),
        )
        .show_inside(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 16.0;
            let query = screen.search.trim().to_lowercase();
            let visible: Vec<usize> = screen
                .transcripts
                .iter()
                .enumerate()
                .filter(|(_, t)| {
                    query.is_empty()
                        || t.title.to_lowercase().contains(&query)
                        || t.preview.to_lowercase().contains(&query)
                })
                .map(|(i, _)| i)
                .collect();

            ui.horizontal(|ui| {
                search_box(
                    ui,
                    theme,
                    &mut screen.search,
                    tr("Search transcripts or Thai text (SQLite FTS)..."),
                    360.0,
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!(
                            "{} {}",
                            screen.transcripts.len(),
                            tr("saved transcripts · Thai (OpenThai ASR)")
                        ))
                        .size(11.5)
                        .color(theme.on_surface_variant),
                    );
                });
            });

            table_frame(ui, theme, |ui| {
                header_row(
                    ui,
                    theme,
                    &COLS,
                    &[
                        tr("Transcript title"),
                        tr("Meeting date"),
                        tr("Duration"),
                        tr("Words"),
                        tr("Actions"),
                    ],
                );

                if visible.is_empty() {
                    ui.add_space(24.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(tr("No transcripts match your search."))
                                .size(13.0)
                                .color(theme.on_surface_variant),
                        );
                    });
                    ui.add_space(24.0);
                    return;
                }

                egui::ScrollArea::vertical()
                    .id_salt("transcripts_table")
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        let last = visible.len() - 1;
                        for (n, &idx) in visible.iter().enumerate() {
                            let entry = &screen.transcripts[idx];
                            body_row(ui, theme, &COLS, 66.0, false, n == last, |cells| {
                                cells.stack(42.0, |ui| {
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(&entry.title)
                                                .size(14.0)
                                                .family(crate::theme::bold_family())
                                                .color(theme.on_surface),
                                        )
                                        .truncate(),
                                    );
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(&entry.preview)
                                                .size(12.5)
                                                .color(theme.on_surface_variant),
                                        )
                                        .truncate(),
                                    );
                                });
                                cells.stack(34.0, |ui| {
                                    let (day, time) =
                                        entry.date.split_once(' ').unwrap_or((&entry.date, ""));
                                    for part in [day, time] {
                                        ui.label(
                                            RichText::new(part)
                                                .font(egui::FontId::monospace(12.0))
                                                .color(theme.on_surface_variant),
                                        );
                                    }
                                });
                                cells.cell(|ui| {
                                    ui.label(
                                        RichText::new(&entry.duration)
                                            .font(egui::FontId::monospace(12.0))
                                            .family(crate::theme::mono_bold_family())
                                            .color(theme.on_surface),
                                    );
                                });
                                cells.cell(|ui| {
                                    ui.label(
                                        RichText::new(&entry.words)
                                            .font(egui::FontId::monospace(12.0))
                                            .color(theme.on_surface),
                                    );
                                });
                                cells.cell(|ui| {
                                    ui.spacing_mut().item_spacing.x = 6.0;
                                    // right-to-left: JSON, SRT, TXT, copy, Open
                                    for fmt in ["JSON", "SRT", "TXT"] {
                                        if mono_chip_button(ui, theme, fmt).clicked() {
                                            toast::show(
                                                ui.ctx(),
                                                crate::i18n::export_not_wired(fmt),
                                            );
                                        }
                                    }
                                    if icon_button(ui, theme, egui_phosphor::regular::COPY, 26.0)
                                        .on_hover_text(tr("Copy transcript"))
                                        .clicked()
                                    {
                                        ui.ctx().copy_text(entry.preview.clone());
                                        toast::show(ui.ctx(), tr("Transcript copied to clipboard"));
                                    }
                                    let variant = if n == 0 {
                                        ButtonVariant::Primary
                                    } else {
                                        ButtonVariant::Secondary
                                    };
                                    if compact_button(ui, theme, None, tr("Open"), variant)
                                        .clicked()
                                    {
                                        navigate = Some(Screen::TranscriptDetail);
                                    }
                                });
                            });
                        }
                    });
            });
        });
    navigate
}
