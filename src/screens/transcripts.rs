use std::path::PathBuf;

use crate::export::ExportFormat;
use crate::i18n::tr;
use crate::library;
use egui::{RichText, Ui};

use crate::app::Screen;
use crate::components::button::{compact_button, icon_button, mono_chip_button, ButtonVariant};
use crate::components::input::search_box;
use crate::components::table::{body_row, header_row, table_frame, Align, Column};
use crate::theme::ThemeColors;
use crate::toast;

#[derive(Clone, Debug, Default)]
pub struct TranscriptEntry {
    pub title: String,
    pub date: String,
    pub duration: String,
    /// Words in the sample rows; the number of lines for real transcripts.
    pub words: String,
    pub preview: String,
    /// The session folder; `None` for the preview's sample rows.
    pub dir: Option<PathBuf>,
    /// Every line, for the search (empty for sample rows).
    pub text: String,
}

/// What a row button asks the app to do (taken once per frame).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TranscriptsRequest {
    Open(PathBuf),
    Copy(PathBuf),
    Export(PathBuf, ExportFormat),
}

pub struct TranscriptsScreen {
    pub search: String,
    pub transcripts: Vec<TranscriptEntry>,
    /// True once the app lists real folders; false keeps the sample rows (preview mode).
    pub live: bool,
    /// False until the first scan arrives in live mode.
    pub loaded: bool,
    /// Set by a row button; taken by the app.
    pub request: Option<TranscriptsRequest>,
}

impl TranscriptsScreen {
    /// Switches to real data; the sample rows go.
    pub fn go_live(&mut self) {
        self.live = true;
        self.loaded = false;
        self.transcripts.clear();
    }

    /// Shows a new scan: the recordings that have a transcript, in the scan's order.
    pub fn set_entries(&mut self, entries: &[library::Entry]) {
        self.loaded = true;
        self.transcripts = entries
            .iter()
            .filter_map(|e| {
                let t = e.transcript.as_ref()?;
                Some(TranscriptEntry {
                    title: e.name.clone(),
                    date: e.stamp.clone(),
                    duration: library::format_duration(e.duration_s),
                    words: t.lines.to_string(),
                    preview: t.preview.clone(),
                    dir: Some(e.dir.clone()),
                    text: t.text.clone(),
                })
            })
            .collect();
    }

    /// Indexes into `transcripts` that match the search (title, preview or any line).
    pub fn visible(&self) -> Vec<usize> {
        let query = self.search.trim().to_lowercase();
        self.transcripts
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                query.is_empty()
                    || t.title.to_lowercase().contains(&query)
                    || t.preview.to_lowercase().contains(&query)
                    || t.text.to_lowercase().contains(&query)
            })
            .map(|(i, _)| i)
            .collect()
    }
}

impl Default for TranscriptsScreen {
    fn default() -> Self {
        Self {
            search: String::new(),
            live: false,
            loaded: true,
            request: None,
            transcripts: vec![
                TranscriptEntry {
                    title: "Q4 Engineering Architecture Sync".to_owned(),
                    date: "2026-11-23 09:30".to_owned(),
                    duration: "00:42:18".to_owned(),
                    words: "3,420".to_owned(),
                    preview: "วันนี้เราจะมาคุยเกี่ยวกับความคืบหน้าของโปรเจกต์ระบบบันทึกการประชุม...".to_owned(),
                    ..Default::default()
                },
                TranscriptEntry {
                    title: "Bangkok Client Onboarding Review".to_owned(),
                    date: "2026-11-22 14:00".to_owned(),
                    duration: "01:08:44".to_owned(),
                    words: "5,890".to_owned(),
                    preview: "ตอนนี้ระบบหลักพัฒนาเสร็จแล้ว เหลือส่วนทดสอบและปรับปรุงประสิทธิภาพ...".to_owned(),
                    ..Default::default()
                },
                TranscriptEntry {
                    title: "Sprint 18 Retrospective".to_owned(),
                    date: "2026-11-18 15:30".to_owned(),
                    duration: "00:28:04".to_owned(),
                    words: "2,180".to_owned(),
                    preview: "สรุปงานในรอบสองสัปดาห์ที่ผ่านมา ปัญหาเรื่องการเชื่อมต่อเสียงและแนวทางแก้ไข..."
                        .to_owned(),
                    ..Default::default()
                },
                TranscriptEntry {
                    title: "WASAPI Integration Design Review".to_owned(),
                    date: "2026-11-15 10:00".to_owned(),
                    duration: "00:55:12".to_owned(),
                    words: "4,310".to_owned(),
                    preview: "การออกแบบ API สำหรับ Windows Audio Session API และการจัดการ Buffer..."
                        .to_owned(),
                    ..Default::default()
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
            let visible = screen.visible();
            let live = screen.live;
            let mut request: Option<TranscriptsRequest> = None;
            if live && !screen.loaded {
                ui.label(
                    RichText::new(tr("Loading transcripts..."))
                        .size(13.0)
                        .color(theme.on_surface_variant),
                );
                return;
            }

            ui.horizontal(|ui| {
                search_box(
                    ui,
                    theme,
                    &mut screen.search,
                    if live {
                        tr("Search transcripts or Thai text...")
                    } else {
                        tr("Search transcripts or Thai text (SQLite FTS)...")
                    },
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
                        if live { tr("Lines") } else { tr("Words") },
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
                                    for (label, format) in [
                                        ("JSON", ExportFormat::Json),
                                        ("SRT", ExportFormat::Srt),
                                        ("TXT", ExportFormat::Txt),
                                    ] {
                                        if mono_chip_button(ui, theme, label).clicked() {
                                            match &entry.dir {
                                                Some(dir) if live => {
                                                    request = Some(TranscriptsRequest::Export(
                                                        dir.clone(),
                                                        format,
                                                    ))
                                                }
                                                _ => toast::show(
                                                    ui.ctx(),
                                                    crate::i18n::export_not_wired(label),
                                                ),
                                            }
                                        }
                                    }
                                    if icon_button(ui, theme, egui_phosphor::regular::COPY, 26.0)
                                        .on_hover_text(tr("Copy transcript"))
                                        .clicked()
                                    {
                                        match &entry.dir {
                                            Some(dir) if live => {
                                                request =
                                                    Some(TranscriptsRequest::Copy(dir.clone()))
                                            }
                                            _ => {
                                                ui.ctx().copy_text(entry.preview.clone());
                                                toast::show(
                                                    ui.ctx(),
                                                    tr("Transcript copied to clipboard"),
                                                );
                                            }
                                        }
                                    }
                                    let variant = if n == 0 {
                                        ButtonVariant::Primary
                                    } else {
                                        ButtonVariant::Secondary
                                    };
                                    if compact_button(ui, theme, None, tr("Open"), variant)
                                        .clicked()
                                    {
                                        match &entry.dir {
                                            Some(dir) if live => {
                                                request =
                                                    Some(TranscriptsRequest::Open(dir.clone()))
                                            }
                                            _ => navigate = Some(Screen::TranscriptDetail),
                                        }
                                    }
                                });
                            });
                        }
                    });
            });
            if request.is_some() {
                screen.request = request;
            }
        });
    navigate
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, text: Option<&str>) -> library::Entry {
        library::Entry {
            dir: PathBuf::from(format!("C:\\rec\\{name}")),
            folder: name.to_owned(),
            name: name.to_owned(),
            stamp: "2026-11-24 09:00".to_owned(),
            duration_s: 65,
            bytes: 1,
            transcript: text.map(|t| library::TranscriptInfo {
                lines: t.lines().count(),
                preview: t.lines().next().unwrap_or("").to_owned(),
                text: t.to_owned(),
            }),
        }
    }

    #[test]
    fn only_recordings_with_a_transcript_are_listed() {
        let mut s = TranscriptsScreen::default();
        s.go_live();
        assert!(s.transcripts.is_empty() && !s.loaded);
        s.set_entries(&[entry("done", Some("สวัสดี\nครับ")), entry("audio-only", None)]);
        assert!(s.loaded);
        assert_eq!(s.transcripts.len(), 1);
        let t = &s.transcripts[0];
        assert_eq!((t.title.as_str(), t.words.as_str()), ("done", "2"));
        assert_eq!(t.duration, "00:01:05");
        assert_eq!(t.preview, "สวัสดี");
    }

    #[test]
    fn search_looks_in_the_title_and_in_every_line() {
        let mut s = TranscriptsScreen::default();
        s.go_live();
        s.set_entries(&[
            entry("standup", Some("เริ่มประชุมกันเลย\nเราจะทำงานนี้ให้เสร็จ")),
            entry("retro", Some("สรุปงานสัปดาห์นี้")),
        ]);
        assert_eq!(s.visible().len(), 2);
        s.search = "ทำ".into();
        assert_eq!(s.visible(), vec![0], "a word from the second line");
        s.search = "RETRO".into();
        assert_eq!(s.visible(), vec![1], "title, any case");
        s.search = "ไม่มี".into();
        assert!(s.visible().is_empty());
    }
}
