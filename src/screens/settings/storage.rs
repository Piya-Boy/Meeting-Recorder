use egui::Ui;

use super::{card, field, page, path_field, row, row_styled};
use crate::components::button::{custom_button, ButtonVariant};
use crate::components::toggle::toggle_switch;
use crate::theme::ThemeColors;
use crate::toast;

pub struct SettingsStorageScreen {
    pub recording_dir: String,
    pub transcript_dir: String,
    pub auto_save: bool,
    pub db_info: String,
    pub cache_info: String,
}

impl Default for SettingsStorageScreen {
    fn default() -> Self {
        Self {
            recording_dir: "C:\\Users\\Nattapong\\Documents\\MeetingRecorder\\Recordings"
                .to_owned(),
            transcript_dir: "C:\\Users\\Nattapong\\Documents\\MeetingRecorder\\Transcripts"
                .to_owned(),
            auto_save: true,
            db_info: "AppData\\Local\\MeetingRecorder\\index.db · 4.8 MB".to_owned(),
            cache_info: "128.4 MB used · Safe to clear when idle".to_owned(),
        }
    }
}

fn dir_field(ui: &mut Ui, theme: &ThemeColors, title: &str, path: &mut String) {
    field(ui, theme, title, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let w = (ui.available_width() - 76.0 - 8.0).max(120.0);
            path_field(ui, theme, path, w);
            if custom_button(ui, theme, None, "Browse", ButtonVariant::Secondary).clicked() {
                toast::show(ui.ctx(), "Folder picker is not wired up yet (UI preview)");
            }
        });
    });
}

pub fn draw_settings_storage(ui: &mut Ui, screen: &mut SettingsStorageScreen, theme: &ThemeColors) {
    page(
        ui,
        theme,
        "Storage",
        "Manage local folders for dual-track WAV recordings, transcripts, SQLite database, and temporary cache.",
        |ui| {
            card(ui, theme, |ui| {
                dir_field(ui, theme, "Recording directory", &mut screen.recording_dir);
                dir_field(ui, theme, "Transcript directory", &mut screen.transcript_dir);

                row(
                    ui,
                    theme,
                    "Automatic saving",
                    Some("Flush audio buffers and transcript segments to disk every 5 seconds."),
                    40.0,
                    |ui| {
                        toggle_switch(ui, theme, &mut screen.auto_save);
                    },
                );

                row_styled(
                    ui,
                    theme,
                    "SQLite database",
                    Some(&screen.db_info),
                    true,
                    None,
                    130.0,
                    |ui| {
                        if custom_button(ui, theme, None, "Optimize DB", ButtonVariant::Secondary)
                            .clicked()
                        {
                            toast::show(ui.ctx(), "Database optimize is not wired up yet (UI preview)");
                        }
                    },
                );
                row_styled(
                    ui,
                    theme,
                    "Temporary cache",
                    Some(&screen.cache_info),
                    true,
                    None,
                    130.0,
                    |ui| {
                        if custom_button(ui, theme, None, "Clear cache", ButtonVariant::Secondary)
                            .clicked()
                        {
                            toast::show(ui.ctx(), "Cache clearing is not wired up yet (UI preview)");
                        }
                    },
                );
            });
        },
    );
}
