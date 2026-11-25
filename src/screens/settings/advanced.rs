use egui::{RichText, Ui};

use super::{card, page, section_title};
use crate::components::button::{compact_button, ButtonVariant};
use crate::i18n::tr;
use crate::theme::ThemeColors;
use crate::toast;
use crate::update::UpdateState;

pub struct SettingsAdvancedScreen {
    /// Mirrors the app's update check (synced by the app each frame).
    pub update: UpdateState,
    /// Set by the "Check for updates" button; taken by the app.
    pub check_requested: bool,
    /// Set by the "Open download page" button; taken by the app.
    pub open_requested: bool,
}

impl Default for SettingsAdvancedScreen {
    fn default() -> Self {
        Self {
            update: UpdateState::Idle,
            check_requested: false,
            open_requested: false,
        }
    }
}

fn not_wired(ui: &Ui, what: &str) {
    toast::show(ui.ctx(), crate::i18n::not_wired(what));
}

/// The line under the version: what the update check found (nothing before the first check).
fn update_line(state: &UpdateState) -> Option<String> {
    Some(match state {
        UpdateState::Idle => return None,
        UpdateState::Checking => tr("Checking for updates...").to_owned(),
        UpdateState::UpToDate => tr("You have the latest version.").to_owned(),
        UpdateState::Available(r) => format!("{} v{}", tr("Update available:"), r.version),
        UpdateState::Failed => tr("Could not check for updates.").to_owned(),
    })
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
        "Version, updates and open-source licences.",
        |ui| {
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
                            RichText::new(format!(
                                "{} {} · Windows 11 x86_64",
                                tr("Version"),
                                crate::update::VERSION
                            ))
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

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    let checking = screen.update == UpdateState::Checking;
                    ui.add_enabled_ui(!checking, |ui| {
                        if compact_button(
                            ui,
                            theme,
                            None,
                            "Check for updates",
                            ButtonVariant::Secondary,
                        )
                        .clicked()
                        {
                            screen.check_requested = true;
                        }
                    });
                    if matches!(screen.update, UpdateState::Available(_))
                        && compact_button(
                            ui,
                            theme,
                            None,
                            "Open download page",
                            ButtonVariant::Primary,
                        )
                        .clicked()
                    {
                        screen.open_requested = true;
                    }
                    if let Some(line) = update_line(&screen.update) {
                        let color = match screen.update {
                            UpdateState::Available(_) => theme.primary,
                            UpdateState::Failed => theme.warning_text,
                            _ => theme.on_surface_variant,
                        };
                        ui.label(RichText::new(line).size(12.0).color(color));
                    }
                });

                let specs = [
                    ("UI Framework", "Rust + egui / eframe"),
                    ("ASR Engine", "OpenThai ASR"),
                    ("Audio Backend", "Windows WASAPI"),
                    ("Storage", "WAV + JSON files"),
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
