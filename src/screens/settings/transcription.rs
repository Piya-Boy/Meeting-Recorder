use egui::{RichText, Ui};

use super::{card, field, page, path_field, pill, row, row_styled, select};
use crate::asr::engine::Language;
use crate::asr::glossary::{self, Glossary};
use crate::asr::model::{self, ModelStatus, Precision};
use crate::asr::worker::TextOptions;
use crate::components::button::{custom_button, ButtonVariant};
use crate::components::toggle::toggle_switch;
use crate::i18n::tr;
use crate::screens::recorder::EngineState;
use crate::theme::ThemeColors;

const RUNTIMES: [&str; 1] = ["ONNX Runtime (CPU)"];
const THREADS: [&str; 3] = ["4 threads", "6 threads", "8 threads"];
const THREAD_COUNTS: [usize; 3] = [4, 6, 8];
const PRECISIONS: [&str; 2] = ["int8 (default, about 1 GB)", "fp32 (about 2.6 GB)"];
const CHUNKS: [&str; 3] = ["500 ms", "1,000 ms", "2,000 ms"];
/// In the order of `Language::ALL`.
const LANGUAGES: [&str; 2] = ["Thai", "Thai + English"];

/// What a model button asks the app to do (taken once per frame by `AppState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelAction {
    Download,
    Cancel,
    Delete,
}

/// The one model button the page shows, and whether it can be pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelButton {
    Download,
    Cancel,
    Delete,
}

pub struct SettingsTranscriptionScreen {
    /// Mirrors the recorder's engine state (synced by the app each frame).
    pub engine: EngineState,
    pub precision: Precision,
    /// What the worker last reported about the model of `precision`.
    pub model_status: ModelStatus,
    /// A transcription is running: the model cannot be deleted under it.
    pub busy: bool,
    /// Set by a button press; taken by the app.
    pub action: Option<ModelAction>,
    pub runtime_idx: usize,
    pub threads_idx: usize,
    pub partial_transcription: bool,
    pub chunk_window_idx: usize,
    pub language_idx: usize,
    /// The word-fix rules as typed, one a line.
    pub glossary_text: String,
}

impl Default for SettingsTranscriptionScreen {
    fn default() -> Self {
        Self {
            engine: EngineState::default(),
            precision: Precision::Int8,
            model_status: ModelStatus::Missing,
            busy: false,
            action: None,
            runtime_idx: 0,
            threads_idx: 0,
            partial_transcription: true,
            chunk_window_idx: 1,
            language_idx: 0,
            glossary_text: String::new(),
        }
    }
}

impl SettingsTranscriptionScreen {
    /// ONNX Runtime threads picked in the select.
    pub fn threads(&self) -> usize {
        THREAD_COUNTS[self.threads_idx.min(THREAD_COUNTS.len() - 1)]
    }

    pub fn language(&self) -> Language {
        Language::from_index(self.language_idx)
    }

    /// The number of word-fix rules and of lines that are not rules.
    pub fn glossary_summary(&self) -> (usize, usize) {
        (
            Glossary::parse(&self.glossary_text).len(),
            glossary::invalid_lines(&self.glossary_text),
        )
    }

    /// What a transcription job or a live run is told about the text.
    pub fn text_options(&self) -> TextOptions {
        TextOptions {
            language: self.language(),
            glossary: Glossary::parse(&self.glossary_text),
        }
    }
}

/// Index of `threads` in the select (an unlisted count gives the default, 4).
/// How often, in milliseconds, new audio is read for the live lines (the "Streaming chunk
/// window" select: 500, 1,000 or 2,000 ms).
pub fn chunk_window_ms(index: usize) -> u64 {
    match index {
        0 => 500,
        2 => 2_000,
        _ => 1_000,
    }
}

pub fn threads_index(threads: u32) -> usize {
    THREAD_COUNTS
        .iter()
        .position(|&n| n as u32 == threads)
        .unwrap_or(0)
}

pub fn engine_pill_text(e: EngineState) -> &'static str {
    match e {
        EngineState::Ready => "OpenThai ASR",
        EngineState::Loading => "Loading model...",
        EngineState::Error => "Engine unavailable",
        EngineState::Missing => "Model not installed",
        EngineState::Downloading => "Downloading model...",
    }
}

pub fn button_state(status: &ModelStatus, busy: bool) -> (ModelButton, bool) {
    match status {
        ModelStatus::Missing | ModelStatus::Failed(_) => (ModelButton::Download, true),
        ModelStatus::Downloading { .. } | ModelStatus::Verifying => (ModelButton::Cancel, true),
        ModelStatus::Ready => (ModelButton::Delete, !busy),
    }
}

pub fn progress_fraction(done: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        (done as f32 / total as f32).clamp(0.0, 1.0)
    }
}

pub fn megabytes(bytes: u64) -> String {
    format!("{} MB", bytes / 1_048_576)
}

/// Opens the model folder in Explorer (only when it exists).
fn open_folder(dir: &std::path::Path) {
    if dir.is_dir() {
        let _ = std::process::Command::new("explorer.exe").arg(dir).spawn();
    }
}

fn progress_bar(ui: &mut Ui, theme: &ThemeColors, fraction: f32) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 6.0), egui::Sense::hover());
    let radius = egui::CornerRadius::same(3);
    ui.painter().rect_filled(rect, radius, theme.outline);
    let mut filled = rect;
    filled.set_width(rect.width() * fraction);
    ui.painter().rect_filled(filled, radius, theme.brand_blue);
}

pub fn draw_settings_transcription(
    ui: &mut Ui,
    screen: &mut SettingsTranscriptionScreen,
    theme: &ThemeColors,
) {
    page(
        ui,
        theme,
        "Transcription",
        "Configure local OpenThai ASR inference, ONNX Runtime threading, and live streaming behavior.",
        |ui| {
            card(ui, theme, |ui| {
                row(
                    ui,
                    theme,
                    "ASR engine",
                    Some("Local-first Thai speech recognition model."),
                    150.0,
                    |ui| match screen.engine {
                        EngineState::Ready => pill(
                            ui,
                            engine_pill_text(screen.engine),
                            theme.primary_container,
                            theme.on_primary_container,
                            None,
                        ),
                        EngineState::Loading | EngineState::Downloading => pill(
                            ui,
                            engine_pill_text(screen.engine),
                            theme.surface_elevated,
                            theme.warning_text,
                            Some(theme.outline),
                        ),
                        EngineState::Missing => pill(
                            ui,
                            engine_pill_text(screen.engine),
                            theme.surface_elevated,
                            theme.on_surface_variant,
                            Some(theme.outline),
                        ),
                        EngineState::Error => pill(
                            ui,
                            engine_pill_text(screen.engine),
                            theme.surface_elevated,
                            theme.error_text,
                            Some(theme.outline),
                        ),
                    },
                );

                let dir = model::model_dir(screen.precision);
                field(ui, theme, "Model folder", |ui| {
                    // The button sits at the right edge; the path takes what is left.
                    let size = egui::vec2(ui.available_width(), 36.0);
                    let layout = egui::Layout::right_to_left(egui::Align::Center);
                    ui.allocate_ui_with_layout(size, layout, |ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        if ui
                            .add_enabled_ui(dir.is_dir(), |ui| {
                                custom_button(
                                    ui,
                                    theme,
                                    None,
                                    "Open folder",
                                    ButtonVariant::Secondary,
                                )
                            })
                            .inner
                            .clicked()
                        {
                            open_folder(&dir);
                        }
                        // Read-only: edits to the copy are thrown away.
                        let mut shown = dir.display().to_string();
                        let w = ui.available_width().max(120.0);
                        path_field(ui, theme, &mut shown, w);
                    });
                });

                row(
                    ui,
                    theme,
                    "Precision",
                    Some("Model weights. fp32 is larger and slower; int8 is the default."),
                    230.0,
                    |ui| {
                        let mut idx = match screen.precision {
                            Precision::Int8 => 0,
                            Precision::Fp32 => 1,
                        };
                        select(ui, theme, "precision", &mut idx, &PRECISIONS, 208.0, false);
                        screen.precision = if idx == 1 {
                            Precision::Fp32
                        } else {
                            Precision::Int8
                        };
                    },
                );

                let (button, enabled) = button_state(&screen.model_status, screen.busy);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 8.0;
                row(
                    ui,
                    theme,
                    "Model",
                    Some("Downloaded once, then kept on this computer."),
                    150.0,
                    |ui| {
                        let (label, variant, action) = match button {
                            ModelButton::Download => {
                                ("Download model", ButtonVariant::Primary, ModelAction::Download)
                            }
                            ModelButton::Cancel => {
                                ("Cancel", ButtonVariant::Secondary, ModelAction::Cancel)
                            }
                            ModelButton::Delete => {
                                ("Delete model", ButtonVariant::Danger, ModelAction::Delete)
                            }
                        };
                        let resp = ui
                            .add_enabled_ui(enabled, |ui| {
                                custom_button(ui, theme, None, label, variant)
                            })
                            .inner;
                        if resp.clicked() {
                            screen.action = Some(action);
                        }
                    },
                );
                match &screen.model_status {
                    ModelStatus::Downloading { done, total } => {
                        progress_bar(ui, theme, progress_fraction(*done, *total));
                        ui.label(
                            RichText::new(format!(
                                "{} / {}",
                                megabytes(*done),
                                megabytes(*total)
                            ))
                            .size(12.0)
                            .color(theme.on_surface_variant),
                        );
                    }
                    ModelStatus::Verifying => {
                        ui.label(
                            RichText::new(tr("Verifying model..."))
                                .size(12.0)
                                .color(theme.on_surface_variant),
                        );
                    }
                    ModelStatus::Failed(e) => {
                        ui.label(
                            RichText::new(format!("{}: {e}", tr("Download failed")))
                                .size(12.0)
                                .color(theme.error_text),
                        );
                    }
                    ModelStatus::Missing => {
                        ui.label(
                            RichText::new(tr("Not downloaded"))
                                .size(12.0)
                                .color(theme.on_surface_variant),
                        );
                    }
                    ModelStatus::Ready => {
                        ui.label(
                            RichText::new(tr("Ready"))
                                .size(12.0)
                                .color(theme.success_text),
                        );
                    }
                }
                });

                row(
                    ui,
                    theme,
                    "Language",
                    Some("Thai writes every word in Thai letters; Thai + English writes English words in Latin letters."),
                    230.0,
                    |ui| {
                        select(
                            ui,
                            theme,
                            "language",
                            &mut screen.language_idx,
                            &LANGUAGES,
                            208.0,
                            true,
                        )
                    },
                );
                row(
                    ui,
                    theme,
                    "Runtime",
                    Some("Execution backend (subject to model compatibility)."),
                    230.0,
                    |ui| {
                        select(
                            ui,
                            theme,
                            "runtime",
                            &mut screen.runtime_idx,
                            &RUNTIMES,
                            208.0,
                            false,
                        )
                    },
                );
                row(
                    ui,
                    theme,
                    "CPU thread configuration",
                    Some("Intra-op worker threads allocated to ONNX inference."),
                    180.0,
                    |ui| {
                        select(
                            ui,
                            theme,
                            "threads",
                            &mut screen.threads_idx,
                            &THREADS,
                            160.0,
                            true,
                        )
                    },
                );
                row(
                    ui,
                    theme,
                    "Partial transcription",
                    Some("Show transcript text while recording. Grey text may still change; a line is final after a pause."),
                    40.0,
                    |ui| {
                        toggle_switch(ui, theme, &mut screen.partial_transcription);
                    },
                );
                row(
                    ui,
                    theme,
                    "Streaming chunk window",
                    Some("How often new audio is read for the live lines."),
                    180.0,
                    |ui| {
                        select(
                            ui,
                            theme,
                            "chunk",
                            &mut screen.chunk_window_idx,
                            &CHUNKS,
                            160.0,
                            true,
                        )
                    },
                );
                ui.label(
                    RichText::new(tr(
                        "Live lines are a preview. The final transcript is made after you stop and may differ a little.",
                    ))
                    .size(12.0)
                    .color(theme.on_surface_variant),
                );

                ui.add_space(4.0);
                ui.label(
                    RichText::new(tr("Word fixes"))
                        .size(14.0)
                        .family(crate::theme::bold_family())
                        .color(theme.on_surface),
                );
                ui.label(
                    RichText::new(tr(
                        "One fix per line: what the transcript says = what it should say. Separate several wrong forms with commas. Applies to transcripts made from now on.",
                    ))
                    .size(12.0)
                    .color(theme.on_surface_variant),
                );
                egui::Frame::NONE
                    .fill(theme.background)
                    .stroke(egui::Stroke::new(1.0, theme.outline))
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut screen.glossary_text)
                                .hint_text("มอล, หมอก = มอก.")
                                .font(egui::FontId::proportional(14.0))
                                .text_color(theme.on_surface)
                                .desired_rows(4)
                                .desired_width(ui.available_width())
                                .frame(false),
                        );
                    });
                let (rules, bad) = screen.glossary_summary();
                let mut summary = format!("{rules} {}", tr("fixes"));
                if bad > 0 {
                    summary.push_str(&format!(
                        " · {bad} {}",
                        tr("lines not understood (use wrong = right)")
                    ));
                }
                ui.label(
                    RichText::new(summary)
                        .size(12.0)
                        .color(if bad > 0 {
                            theme.warning_text
                        } else {
                            theme.on_surface_variant
                        }),
                );

                // Disabled, future feature
                let mut never = false;
                row_styled(
                    ui,
                    theme,
                    "Speaker diarization",
                    Some("Not available in this release. Planned for a future update."),
                    false,
                    Some("Future feature"),
                    40.0,
                    |ui| {
                        ui.add_enabled_ui(false, |ui| {
                            toggle_switch(ui, theme, &mut never);
                        });
                    },
                );
            });
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threads_follow_the_select_and_default_to_4() {
        let mut s = SettingsTranscriptionScreen::default();
        assert_eq!(s.threads(), 4);
        for (idx, n) in [(0, 4), (1, 6), (2, 8)] {
            s.threads_idx = idx;
            assert_eq!(s.threads(), n);
        }
    }

    #[test]
    fn engine_pill_has_a_thai_text_for_every_state() {
        use crate::screens::recorder::EngineState::*;
        for e in [Missing, Downloading, Loading, Ready, Error] {
            assert!(crate::i18n::has_thai(engine_pill_text(e)), "{e:?}");
        }
    }

    #[test]
    fn download_and_delete_buttons_follow_the_model_status() {
        use crate::asr::model::ModelStatus::*;
        let b = |st, busy| button_state(&st, busy);
        assert_eq!(b(Missing, false), (ModelButton::Download, true));
        assert_eq!(b(Failed("x".into()), false), (ModelButton::Download, true));
        assert_eq!(
            b(Downloading { done: 1, total: 2 }, false),
            (ModelButton::Cancel, true)
        );
        assert_eq!(b(Verifying, false), (ModelButton::Cancel, true));
        assert_eq!(b(Ready, false), (ModelButton::Delete, true));
        assert_eq!(b(Ready, true), (ModelButton::Delete, false));
    }

    #[test]
    fn progress_helpers() {
        assert_eq!(progress_fraction(0, 0), 0.0);
        assert_eq!(progress_fraction(50, 100), 0.5);
        assert_eq!(progress_fraction(200, 100), 1.0);
        assert_eq!(megabytes(883_000_000), "842 MB");
    }

    #[test]
    fn the_chunk_window_index_maps_to_milliseconds() {
        assert_eq!(chunk_window_ms(0), 500);
        assert_eq!(chunk_window_ms(1), 1_000);
        assert_eq!(chunk_window_ms(2), 2_000);
        assert_eq!(
            chunk_window_ms(9),
            1_000,
            "an unknown index falls back to the default"
        );
    }

    #[test]
    fn the_language_select_and_the_word_fixes_summary() {
        use crate::asr::engine::Language;
        let mut s = SettingsTranscriptionScreen::default();
        assert_eq!(s.language(), Language::Thai);
        s.language_idx = 1;
        assert_eq!(s.language(), Language::ThaiEnglish);
        s.glossary_text = "มอล, หมอก = มอก.\nไม่ใช่กฎ\n".to_owned();
        assert_eq!(s.glossary_summary(), (2, 1));
        assert_eq!(s.text_options().glossary.apply("มอล"), "มอก.");
        assert_eq!(s.text_options().language, Language::ThaiEnglish);
    }
}
