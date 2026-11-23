use egui::{RichText, Ui};

use super::{card, field, page, path_field, row_styled, section_title};
use crate::components::button::{compact_button, custom_button, ButtonVariant};
use crate::theme::ThemeColors;
use crate::toast;

pub struct SettingsAdvancedScreen {
    pub log_dir: String,
    pub model_cache_info: String,
    pub perf_info: String,
}

impl Default for SettingsAdvancedScreen {
    fn default() -> Self {
        Self {
            log_dir: "C:\\Users\\Nattapong\\AppData\\Local\\MeetingRecorder\\logs".to_owned(),
            model_cache_info: "642.0 MB · OpenThai ASR ONNX weights".to_owned(),
            perf_info: "UI: 60 FPS (egui) · WASAPI buffer: 10 ms · Memory: 312 MB".to_owned(),
        }
    }
}

fn not_wired(ui: &Ui, what: &str) {
    toast::show(ui.ctx(), crate::i18n::not_wired(what));
}

pub fn draw_settings_advanced(
    ui: &mut Ui,
    screen: &mut SettingsAdvancedScreen,
    theme: &ThemeColors,
) {
    page(
        ui,
        theme,
        "Advanced",
        "Diagnostics, log files, model cache, and runtime performance metrics.",
        |ui| {
            card(ui, theme, |ui| {
                field(ui, theme, "Log directory", |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        let w = (ui.available_width() - 100.0 - 8.0).max(120.0);
                        path_field(ui, theme, &mut screen.log_dir, w);
                        if custom_button(ui, theme, None, "Open folder", ButtonVariant::Secondary)
                            .clicked()
                        {
                            not_wired(ui, "Opening the log folder");
                        }
                    });
                });
                row_styled(
                    ui,
                    theme,
                    "Model cache",
                    Some(&screen.model_cache_info),
                    true,
                    None,
                    130.0,
                    |ui| {
                        if custom_button(
                            ui,
                            theme,
                            None,
                            "Verify checksum",
                            ButtonVariant::Secondary,
                        )
                        .clicked()
                        {
                            not_wired(ui, "Checksum verification");
                        }
                    },
                );
                row_styled(
                    ui,
                    theme,
                    "Performance information",
                    Some(&screen.perf_info),
                    true,
                    None,
                    150.0,
                    |ui| {
                        if custom_button(
                            ui,
                            theme,
                            None,
                            "Export diagnostics",
                            ButtonVariant::Secondary,
                        )
                        .clicked()
                        {
                            not_wired(ui, "Diagnostics export");
                        }
                    },
                );
            });

            section_title(ui, theme, "About");
            card(ui, theme, |ui| {
                ui.spacing_mut().item_spacing.y = 16.0;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    let (logo, _) =
                        ui.allocate_exact_size(egui::Vec2::splat(36.0), egui::Sense::hover());
                    if !crate::branding::paint_logo(ui, logo) {
                        ui.painter()
                            .rect_filled(logo, egui::CornerRadius::same(6), theme.primary);
                        ui.painter().text(
                            logo.center(),
                            egui::Align2::CENTER_CENTER,
                            egui_phosphor::regular::MICROPHONE,
                            egui::FontId::proportional(20.0),
                            theme.on_primary,
                        );
                    }
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(
                            RichText::new("Meeting Recorder")
                                .size(14.0)
                                .family(crate::theme::bold_family())
                                .color(theme.on_surface),
                        );
                        ui.label(
                            RichText::new(crate::i18n::tr("Version 0.4.2 · Windows 11 x86_64"))
                                .font(egui::FontId::monospace(12.0))
                                .color(theme.on_surface_variant),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if compact_button(
                            ui,
                            theme,
                            None,
                            "Open-source licenses",
                            ButtonVariant::Secondary,
                        )
                        .clicked()
                        {
                            not_wired(ui, "The licenses viewer");
                        }
                    });
                });

                let specs = [
                    ("UI Framework", "Rust + egui / eframe"),
                    ("ASR Engine", "OpenThai ASR"),
                    ("Audio Backend", "Windows WASAPI"),
                    ("Storage", "SQLite + ONNX Runtime"),
                ];
                egui::Grid::new("about_specs")
                    .num_columns(2)
                    .spacing([24.0, 12.0])
                    .min_col_width(ui.available_width() / 2.0 - 12.0)
                    .show(ui, |ui| {
                        for (i, (k, v)) in specs.iter().enumerate() {
                            ui.label(
                                RichText::new(format!("{}: {v}", crate::i18n::tr(k)))
                                    .font(egui::FontId::monospace(12.0))
                                    .color(theme.on_surface_variant),
                            );
                            if i % 2 == 1 {
                                ui.end_row();
                            }
                        }
                    });
            });
        },
    );
}
