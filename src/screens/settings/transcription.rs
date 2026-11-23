use egui::Ui;

use super::{card, field, page, path_field, pill, row, row_styled, select};
use crate::components::button::{custom_button, ButtonVariant};
use crate::components::toggle::toggle_switch;
use crate::theme::ThemeColors;
use crate::toast;

const RUNTIMES: [&str; 2] = ["ONNX Runtime (CPU)", "ONNX Runtime (DirectML)"];
const THREADS: [&str; 3] = ["4 threads", "6 threads (Auto)", "8 threads"];
const CHUNKS: [&str; 3] = ["500 ms", "1,000 ms", "2,000 ms"];

pub struct SettingsTranscriptionScreen {
    /// Mirrors the recorder's engine state (synced by the app each frame).
    pub engine: crate::screens::recorder::EngineState,
    pub runtime_idx: usize,
    pub threads_idx: usize,
    pub partial_transcription: bool,
    pub chunk_window_idx: usize,
    pub model_path: String,
}

impl Default for SettingsTranscriptionScreen {
    fn default() -> Self {
        Self {
            engine: crate::screens::recorder::EngineState::default(),
            runtime_idx: 0,
            threads_idx: 1,
            partial_transcription: true,
            chunk_window_idx: 1,
            model_path: "C:\\ProgramData\\MeetingRecorder\\models\\openthaigpt-asr-v2.onnx"
                .to_owned(),
        }
    }
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
                    110.0,
                    |ui| {
                        use crate::screens::recorder::EngineState;
                        match screen.engine {
                            EngineState::Ready => pill(
                                ui,
                                "OpenThai ASR",
                                theme.primary_container,
                                theme.on_primary_container,
                                None,
                            ),
                            EngineState::Loading => pill(
                                ui,
                                "Loading model...",
                                theme.surface_elevated,
                                theme.warning_text,
                                Some(theme.outline),
                            ),
                            EngineState::Error => pill(
                                ui,
                                "Engine unavailable",
                                theme.surface_elevated,
                                theme.error_text,
                                Some(theme.outline),
                            ),
                        }
                    },
                );

                field(ui, theme, "Model path", |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        let w = (ui.available_width() - 76.0 - 8.0).max(120.0);
                        path_field(ui, theme, &mut screen.model_path, w);
                        if custom_button(ui, theme, None, "Browse", ButtonVariant::Secondary)
                            .clicked()
                        {
                            toast::show(ui.ctx(), "File picker is not wired up yet (UI preview)");
                        }
                    });
                });

                row(
                    ui,
                    theme,
                    "Language",
                    Some("Primary recognition language."),
                    90.0,
                    |ui| {
                        pill(
                            ui,
                            "Thai (TH)",
                            theme.surface_elevated,
                            theme.on_surface,
                            Some(theme.outline),
                        )
                    },
                );
                row(
                    ui,
                    theme,
                    "Runtime",
                    Some("Execution backend (subject to model compatibility)."),
                    230.0,
                    |ui| select(ui, theme, "runtime", &mut screen.runtime_idx, &RUNTIMES, 208.0, false),
                );
                row(
                    ui,
                    theme,
                    "CPU thread configuration",
                    Some("Intra-op worker threads allocated to ONNX inference."),
                    180.0,
                    |ui| select(ui, theme, "threads", &mut screen.threads_idx, &THREADS, 160.0, true),
                );
                row(
                    ui,
                    theme,
                    "Partial transcription",
                    Some("Display interim Thai text while speech is still being processed."),
                    40.0,
                    |ui| {
                        toggle_switch(ui, theme, &mut screen.partial_transcription);
                    },
                );
                row(
                    ui,
                    theme,
                    "Streaming chunk window",
                    Some("Audio buffer interval before running incremental ASR decoding."),
                    180.0,
                    |ui| select(ui, theme, "chunk", &mut screen.chunk_window_idx, &CHUNKS, 160.0, true),
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
