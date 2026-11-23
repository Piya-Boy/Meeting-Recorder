//! Text inputs shared across screens.

use egui::{RichText, Ui};

use crate::theme::ThemeColors;

/// Search field: magnifier icon + borderless text edit inside a bordered 32px box.
pub fn search_box(ui: &mut Ui, theme: &ThemeColors, text: &mut String, hint: &str, width: f32) {
    egui::Frame::NONE
        .fill(theme.surface)
        .stroke(egui::Stroke::new(1.0, theme.outline))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(10, 0))
        .show(ui, |ui| {
            ui.set_height(30.0);
            ui.set_width(width);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(egui_phosphor::regular::MAGNIFYING_GLASS)
                        .size(15.0)
                        .color(theme.on_surface_variant),
                );
                ui.add(
                    egui::TextEdit::singleline(text)
                        .hint_text(crate::i18n::tr(hint))
                        .desired_width(ui.available_width())
                        .text_color(theme.on_surface)
                        .frame(false),
                );
            });
        });
}
