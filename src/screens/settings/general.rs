use egui::{RichText, Ui};

use super::{card, page, row, section_title, select};
use crate::components::toggle::toggle_switch;
use crate::theme::ThemeColors;

const LANGUAGES: [&str; 2] = ["English (US)", "ไทย (Thai)"];

const SHORTCUTS: [(&str, &str); 4] = [
    ("Start / Stop recording", "Ctrl + Shift + R"),
    ("Pause / Resume recording", "Ctrl + Shift + P"),
    ("Toggle auto-scroll", "Ctrl + Shift + A"),
    ("Copy transcript", "Ctrl + Shift + C"),
];

pub struct SettingsGeneralScreen {
    pub language_idx: usize,
    pub confirm_delete: bool,
    pub check_updates: bool,
}

impl Default for SettingsGeneralScreen {
    fn default() -> Self {
        Self {
            language_idx: 0,
            confirm_delete: true,
            check_updates: true,
        }
    }
}

pub fn draw_settings_general(ui: &mut Ui, screen: &mut SettingsGeneralScreen, theme: &ThemeColors) {
    page(
        ui,
        theme,
        "General",
        "Language, behaviour and keyboard shortcuts. The shortcuts work while this window has focus.",
        |ui| {
            card(ui, theme, |ui| {
                row(
                    ui,
                    theme,
                    "Application language",
                    Some("Interface display language (transcription language is set under Transcription)."),
                    180.0,
                    |ui| select(ui, theme, "app_language", &mut screen.language_idx, &LANGUAGES, 160.0, false),
                );
                row(
                    ui,
                    theme,
                    "Confirm before deleting recordings",
                    Some("Show a confirmation dialog before permanently removing audio files."),
                    40.0,
                    |ui| {
                        toggle_switch(ui, theme, &mut screen.confirm_delete);
                    },
                );
                row(
                    ui,
                    theme,
                    "Check for updates",
                    Some("Asks GitHub once a day whether a newer version exists. No recording or personal data is sent."),
                    40.0,
                    |ui| {
                        toggle_switch(ui, theme, &mut screen.check_updates);
                    },
                );
            });

            section_title(ui, theme, "Keyboard shortcuts");
            card(ui, theme, |ui| {
                ui.spacing_mut().item_spacing.y = 18.0;
                for (action, keys) in SHORTCUTS {
                    row(ui, theme, action, None, 140.0, |ui| {
                        kbd_chip(ui, theme, keys)
                    });
                }
            });
        },
    );
}

fn kbd_chip(ui: &mut Ui, theme: &ThemeColors, keys: &str) {
    egui::Frame::NONE
        .fill(theme.background)
        .stroke(egui::Stroke::new(1.0, theme.outline))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(keys)
                        .font(egui::FontId::monospace(12.0))
                        .family(crate::theme::mono_bold_family())
                        .color(theme.on_surface),
                );
            });
        });
}
