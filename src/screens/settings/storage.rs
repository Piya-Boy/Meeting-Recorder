use egui::{RichText, Ui};

use super::{card, field, page, path_field};
use crate::i18n::tr;
use crate::theme::ThemeColors;

/// Where recordings go. Read-only: there is no folder picker yet.
pub fn draw_settings_storage(ui: &mut Ui, recording_dir: &str, theme: &ThemeColors) {
    page(
        ui,
        theme,
        "Storage",
        "Where recordings and transcripts are saved.",
        |ui| {
            card(ui, theme, |ui| {
                field(ui, theme, "Recording directory", |ui| {
                    // `path_field` edits this copy, so typing in it changes nothing.
                    let mut shown = recording_dir.to_owned();
                    let width = ui.available_width();
                    path_field(ui, theme, &mut shown, width);
                });
                ui.add(
                    egui::Label::new(
                        RichText::new(tr(
                            "Each recording has its own folder with the audio tracks (WAV) and, once transcribed, the transcript files.",
                        ))
                        .size(13.0)
                        .color(theme.on_surface_variant),
                    )
                    .wrap(),
                );
            });
        },
    );
}
