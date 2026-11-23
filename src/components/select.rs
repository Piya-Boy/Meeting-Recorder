//! Dropdown matching Stitch's `<select>`: 36px tall (`px-3 py-2` + 1px border), 6px radius,
//! recessed fill, thin chevron on the right.

use egui::{Color32, CornerRadius, Stroke, Ui};

use crate::theme::ThemeColors;

/// Height of every dropdown (13px label line + 2 × 8px padding + border).
pub const SELECT_HEIGHT: f32 = 36.0;

/// Draws a dropdown over `options`; `fill` is the control background
/// (`theme.background` for form fields, `theme.surface` for toolbars).
#[allow(clippy::too_many_arguments)]
pub fn dropdown<S: AsRef<str>>(
    ui: &mut Ui,
    theme: &ThemeColors,
    id: &str,
    index: &mut usize,
    options: &[S],
    width: f32,
    fill: Color32,
    mono: bool,
) {
    if options.is_empty() {
        return;
    }
    let label = |s: &str| {
        if mono {
            egui::RichText::new(crate::i18n::tr(s)).font(egui::FontId::monospace(12.0))
        } else {
            egui::RichText::new(crate::i18n::tr(s)).size(13.0)
        }
        .color(theme.on_surface)
    };

    ui.scope(|ui| {
        let radius = CornerRadius::same(6);
        let outline = Stroke::new(1.0, theme.outline);
        {
            let style = ui.style_mut();
            style.spacing.interact_size.y = SELECT_HEIGHT;
            style.spacing.button_padding = egui::vec2(12.0, 8.0);
            let w = &mut style.visuals.widgets;
            for (state, bg, stroke) in [
                (&mut w.inactive, fill, outline),
                (
                    &mut w.hovered,
                    fill,
                    Stroke::new(1.0, theme.on_surface_variant),
                ),
                (&mut w.active, fill, outline),
                (&mut w.open, fill, Stroke::new(1.0, theme.brand_blue)),
            ] {
                state.bg_fill = bg;
                state.weak_bg_fill = bg;
                state.bg_stroke = stroke;
                state.corner_radius = radius;
                state.expansion = 0.0;
            }
        }

        let chevron_color = theme.on_surface_variant;
        egui::ComboBox::from_id_salt(id)
            .width(width.max(40.0))
            .wrap_mode(egui::TextWrapMode::Truncate)
            .selected_text(label(options[(*index).min(options.len() - 1)].as_ref()))
            .icon(move |ui, rect, _visuals, is_open, _above_or_below| {
                let c = rect.center();
                let (w, h) = (4.0, 2.5);
                let dy = if is_open { h } else { -h };
                let stroke = Stroke::new(1.5, chevron_color);
                ui.painter().line_segment(
                    [egui::pos2(c.x - w, c.y + dy), egui::pos2(c.x, c.y - dy)],
                    stroke,
                );
                ui.painter().line_segment(
                    [egui::pos2(c.x, c.y - dy), egui::pos2(c.x + w, c.y + dy)],
                    stroke,
                );
            })
            .show_ui(ui, |ui| {
                ui.spacing_mut().button_padding = egui::vec2(10.0, 6.0);
                for (i, opt) in options.iter().enumerate() {
                    ui.selectable_value(index, i, label(opt.as_ref()));
                }
            });
    });
}
