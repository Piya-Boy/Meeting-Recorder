use crate::asr::transcript::{format_clock, Line, Track};
use crate::i18n::tr;
use crate::screens::recorder::tinted_badge;
use egui::{RichText, Ui};

use crate::theme::ThemeColors;

#[derive(Clone, Debug)]
pub struct TranscriptArticle {
    pub timestamp: String,
    pub text: String,
    /// Who spoke; `None` for the preview's sample paragraphs.
    pub speaker: Option<Track>,
}

pub struct TranscriptDetailScreen {
    pub title: String,
    pub date: String,
    pub duration: String,
    pub word_count: String,
    pub articles: Vec<TranscriptArticle>,
    /// True once a real transcript is shown (read-only, with speakers).
    pub live: bool,
    /// The recording's folder for a real transcript (the export buttons use it).
    pub dir: Option<std::path::PathBuf>,
}

impl TranscriptDetailScreen {
    /// Shows a real transcript.
    pub fn show_real(
        &mut self,
        title: String,
        date: String,
        duration: String,
        dir: std::path::PathBuf,
        lines: &[Line],
    ) {
        self.live = true;
        self.dir = Some(dir);
        self.title = title;
        self.date = date;
        self.duration = duration;
        self.word_count = String::new();
        self.articles = lines
            .iter()
            .map(|l| TranscriptArticle {
                timestamp: format_clock(l.start_ms),
                text: l.text.clone(),
                speaker: Some(l.track),
            })
            .collect();
    }
}

impl Default for TranscriptDetailScreen {
    fn default() -> Self {
        Self {
            live: false,
            dir: None,
            title: "Q4 Engineering Architecture Sync".to_owned(),
            date: "2026-11-23 09:30".to_owned(),
            duration: "00:42:18".to_owned(),
            word_count: "3,420 words".to_owned(),
            articles: vec![
                TranscriptArticle {
                    timestamp: "00:00:12".to_owned(),
                    text: "สวัสดีครับทุกคน ขอเริ่มประชุมได้เลยนะครับ วันนี้เราจะมาพูดคุยกันเรื่อง Architecture ของระบบในไตรมาสที่ 4 ซึ่งมีหลายส่วนที่ต้องตัดสินใจกัน".to_owned(),
                    speaker: None,
                },
                TranscriptArticle {
                    timestamp: "00:01:45".to_owned(),
                    text: "ก่อนอื่นขอสรุปสิ่งที่ทำได้ในไตรมาสที่แล้วก่อนนะครับ เราได้ migrate ระบบ backend ไปอยู่บน Kubernetes เรียบร้อยแล้ว และ latency ลดลงไปประมาณ 23% ซึ่งดีกว่าที่คาดไว้มาก".to_owned(),
                    speaker: None,
                },
                TranscriptArticle {
                    timestamp: "00:03:22".to_owned(),
                    text: "สำหรับ Q4 เราจะโฟกัสที่สามส่วนหลักคือ Service Mesh, Observability Stack และ Database Sharding Strategy ทั้งสามส่วนนี้มีความเชื่อมโยงกันอยู่ครับ".to_owned(),
                    speaker: None,
                },
                TranscriptArticle {
                    timestamp: "00:05:10".to_owned(),
                    text: "เรื่อง Service Mesh นั้น เราได้ evaluate ระหว่าง Istio กับ Linkerd ครับ Istio มี feature ที่ครบกว่าแต่ resource usage สูงกว่า ส่วน Linkerd เบากว่าและ setup ง่ายกว่ามาก".to_owned(),
                    speaker: None,
                },
                TranscriptArticle {
                    timestamp: "00:08:33".to_owned(),
                    text: "ทีม Platform แนะนำ Linkerd ครับ เพราะ cluster ของเราไม่ได้ใหญ่มาก และ feature ของ Linkerd ก็เพียงพอสำหรับ use case ของเรา ประหยัด resource ไปได้มากด้วย".to_owned(),
                    speaker: None,
                },
                TranscriptArticle {
                    timestamp: "00:11:20".to_owned(),
                    text: "Observability Stack นั้นเราจะใช้ Prometheus กับ Grafana เป็นหลักครับ เพิ่ม Loki สำหรับ log aggregation และ Tempo สำหรับ distributed tracing เพื่อให้ครบ three pillars of observability".to_owned(),
                    speaker: None,
                },
                TranscriptArticle {
                    timestamp: "00:15:44".to_owned(),
                    text: "สำหรับ Database Sharding เรากำลัง evaluate อยู่ครับ ตอนนี้มีสองแนวทางคือ Horizontal Sharding ด้วย PlanetScale หรือจะ self-manage บน PostgreSQL โดยใช้ Citus".to_owned(),
                    speaker: None,
                },
                TranscriptArticle {
                    timestamp: "00:19:05".to_owned(),
                    text: "ข้อดีของ PlanetScale คือ managed service ไม่ต้องดูแลเอง แต่ค่าใช้จ่ายสูงกว่า ส่วน Citus ถ้า self-manage ได้จะประหยัดกว่า แต่ต้องการ expertise มากกว่า".to_owned(),
                    speaker: None,
                },
                TranscriptArticle {
                    timestamp: "00:23:18".to_owned(),
                    text: "ในส่วนของ Timeline นั้น ภายใน Q4 เราตั้งเป้าว่าจะ rollout Service Mesh ให้ครบทุก production cluster ภายในเดือน 10 Observability Stack ภายในเดือน 11 และ Database Sharding planning ภายในเดือน 12".to_owned(),
                    speaker: None,
                },
                TranscriptArticle {
                    timestamp: "00:28:50".to_owned(),
                    text: "มีคำถามอะไรไหมครับ? โดยเฉพาะเรื่อง resource allocation สำหรับ Q4 นี้ เราต้องการ engineer อีกกี่คน และ budget ที่ต้องใช้ประมาณเท่าไหร่ครับ".to_owned(),
                    speaker: None,
                },
                TranscriptArticle {
                    timestamp: "00:32:15".to_owned(),
                    text: "ถ้าทำ Service Mesh ด้วย Linkerd เราต้องการ engineer ที่มี experience กับ Kubernetes networking ครับ ตอนนี้ทีม Platform มีอยู่สองคนที่ทำได้ แต่จะ stretched มาก ถ้าทำพร้อมกันหมดทุกอย่าง".to_owned(),
                    speaker: None,
                },
                TranscriptArticle {
                    timestamp: "00:38:42".to_owned(),
                    text: "สรุปแล้วกัน เราจะเริ่มด้วย Linkerd ก่อน ทำ Observability Stack ต่อ แล้วค่อยตัดสินใจเรื่อง Database ในเดือน 11 เมื่อเรามีข้อมูล load ที่ชัดเจนกว่านี้ครับ ขอบคุณทุกคนนะครับ".to_owned(),
                    speaker: None,
                },
            ],
        }
    }
}

/// Plain-text rendering of the transcript (used by "Copy all"): `[hh:mm:ss] <who>: <text>`,
/// or `[hh:mm:ss] <text>` for a paragraph without a speaker.
pub fn transcript_text_with(articles: &[TranscriptArticle], you: &str, others: &str) -> String {
    articles
        .iter()
        .map(|a| match a.speaker {
            Some(Track::Mic) => format!("[{}] {}: {}", a.timestamp, you, a.text),
            Some(Track::System) => format!("[{}] {}: {}", a.timestamp, others, a.text),
            None => format!("[{}] {}", a.timestamp, a.text),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The same text for lines that are not on the detail page (the list's Copy button).
pub fn lines_text(lines: &[Line]) -> String {
    let articles: Vec<TranscriptArticle> = lines
        .iter()
        .map(|l| TranscriptArticle {
            timestamp: format_clock(l.start_ms),
            text: l.text.clone(),
            speaker: Some(l.track),
        })
        .collect();
    transcript_text_with(&articles, tr("You"), tr("Others"))
}

pub fn transcript_as_text(screen: &TranscriptDetailScreen) -> String {
    transcript_text_with(&screen.articles, tr("You"), tr("Others"))
}

/// Draws the transcript document viewer. Header actions live in the app top bar.
pub fn draw_transcript_detail_screen(
    ui: &mut Ui,
    theme: &ThemeColors,
    screen: &mut TranscriptDetailScreen,
) {
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(theme.background))
        .show_inside(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("transcript_detail_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    // Centre a column up to 768px wide.
                    let avail = ui.available_width();
                    let col_w = (avail - 48.0).clamp(320.0, 768.0);
                    let side = ((avail - col_w) / 2.0).max(0.0);
                    ui.horizontal_top(|ui| {
                        ui.add_space(side);
                        ui.vertical(|ui| {
                            ui.set_width(col_w);
                            ui.add_space(24.0);
                            ui.spacing_mut().item_spacing.y = 16.0;
                            draw_meta_card(ui, theme, screen);
                            draw_document_card(ui, theme, screen);
                            ui.add_space(24.0);
                        });
                    });
                });
        });
}

fn draw_meta_card(ui: &mut Ui, theme: &ThemeColors, screen: &TranscriptDetailScreen) {
    egui::Frame::NONE
        .fill(theme.surface)
        .stroke(egui::Stroke::new(1.0, theme.outline))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    let tail = if screen.live {
                        format!("{} {}", screen.articles.len(), tr("lines"))
                    } else {
                        screen.word_count.replace(" words", tr(" words"))
                    };
                    ui.label(
                        RichText::new(format!(
                            "{}  ·  {}  ·  {}",
                            screen.date, screen.duration, tail
                        ))
                        .font(egui::FontId::monospace(12.0))
                        .color(theme.on_surface_variant),
                    );
                    if !screen.live {
                        ui.label(
                            RichText::new(
                                tr("Click any paragraph below to edit text inline. Changes auto-save to SQLite."),
                            )
                            .size(13.0)
                            .color(theme.on_surface_variant),
                        );
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    egui::Frame::NONE
                        .fill(theme.primary_container)
                        .corner_radius(egui::CornerRadius::same(6))
                        .inner_margin(egui::Margin::symmetric(8, 3))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(format!(
                                    "{}  {}",
                                    egui_phosphor::regular::TRANSLATE,
                                    tr("OpenThai ASR · Thai")
                                ))
                                .size(11.5)
                                .family(crate::theme::bold_family())
                                .color(theme.on_primary_container),
                            );
                        });
                });
            });
        });
}

fn draw_document_card(ui: &mut Ui, theme: &ThemeColors, screen: &mut TranscriptDetailScreen) {
    egui::Frame::NONE
        .fill(theme.surface)
        .stroke(egui::Stroke::new(1.0, theme.outline))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(8, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            let live = screen.live;
            for article in screen.articles.iter_mut() {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    ui.add_space(8.0);
                    ui.add_sized(
                        [64.0, 26.0],
                        egui::Label::new(
                            RichText::new(&article.timestamp)
                                .font(egui::FontId::monospace(12.0))
                                .family(crate::theme::mono_bold_family())
                                .color(theme.on_surface),
                        ),
                    );
                    if live {
                        ui.allocate_ui(egui::vec2(64.0, 26.0), |ui| {
                            ui.set_min_size(egui::vec2(64.0, 26.0));
                            ui.add_space(2.0);
                            match article.speaker {
                                Some(Track::Mic) => {
                                    tinted_badge(ui, theme.brand_blue, theme.link_text, tr("You"))
                                }
                                Some(Track::System) => tinted_badge(
                                    ui,
                                    theme.outline,
                                    theme.on_surface_variant,
                                    tr("Others"),
                                ),
                                None => {}
                            }
                        });
                        egui::Frame::NONE
                            .inner_margin(egui::Margin {
                                left: 0,
                                right: 12,
                                top: 3,
                                bottom: 3,
                            })
                            .show(ui, |ui| {
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(&article.text)
                                            .size(15.0)
                                            .color(theme.on_surface),
                                    )
                                    .wrap(),
                                );
                            });
                        return;
                    }
                    let w = ui.available_width() - 8.0;
                    ui.add(
                        egui::TextEdit::multiline(&mut article.text)
                            .font(egui::FontId::proportional(15.0))
                            .text_color(theme.on_surface)
                            .desired_rows(1)
                            .desired_width(w)
                            .frame(false)
                            .margin(egui::Margin::symmetric(4, 10)),
                    );
                });
                if live {
                    ui.add_space(10.0);
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_all_text_has_one_timestamped_line_per_article() {
        let screen = TranscriptDetailScreen::default();
        let text = transcript_as_text(&screen);
        assert_eq!(text.lines().count(), screen.articles.len());
        assert!(text.starts_with("[00:00:12] "));
    }

    fn line(track: Track, start_ms: u64, text: &str) -> Line {
        Line {
            track,
            start_ms,
            end_ms: start_ms + 500,
            text: text.to_owned(),
        }
    }

    #[test]
    fn show_real_remembers_the_folder_and_the_sample_has_none() {
        assert_eq!(TranscriptDetailScreen::default().dir, None);
        let mut s = TranscriptDetailScreen::default();
        let dir = std::path::PathBuf::from("C:\\rec\\standup");
        s.show_real("t".into(), "d".into(), "00:00:01".into(), dir.clone(), &[]);
        assert_eq!(s.dir, Some(dir));
    }

    #[test]
    fn real_lines_become_timestamped_articles_with_speakers() {
        let mut s = TranscriptDetailScreen::default();
        s.show_real(
            "standup".into(),
            "2026-11-24 09:00".into(),
            "00:01:05".into(),
            std::path::PathBuf::from("C:\\rec\\standup"),
            &[
                line(Track::Mic, 0, "สวัสดีครับ"),
                line(Track::System, 3_661_000, "ครับผม"),
            ],
        );
        assert!(s.live);
        assert_eq!(s.title, "standup");
        assert_eq!(s.articles.len(), 2);
        assert_eq!(s.articles[1].timestamp, "01:01:01");
        assert_eq!(s.articles[1].speaker, Some(Track::System));
        let text = transcript_text_with(&s.articles, "You", "Others");
        assert_eq!(text, "[00:00:00] You: สวัสดีครับ\n[01:01:01] Others: ครับผม");
    }
}
