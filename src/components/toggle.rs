use crate::theme::ThemeColors;
use egui::{vec2, CornerRadius, Pos2, Response, Stroke, Ui};

pub fn toggle_switch(ui: &mut Ui, theme: &ThemeColors, value: &mut bool) -> Response {
    let width = 36.0;
    let height = 20.0;
    let knob_radius = 7.0;

    let (rect, mut response) = ui.allocate_exact_size(vec2(width, height), egui::Sense::click());

    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }

    if ui.is_rect_visible(rect) {
        let is_checked = *value;
        let painter = ui.painter();

        let track_color = if is_checked {
            theme.primary
        } else {
            theme.surface_elevated
        };

        let stroke = if is_checked {
            Stroke::NONE
        } else {
            Stroke::new(1.0, theme.outline)
        };

        // Draw track pill
        painter.rect(
            rect,
            CornerRadius::same((height / 2.0) as u8),
            track_color,
            stroke,
            egui::StrokeKind::Inside,
        );

        // Knob position
        let x_pos = if is_checked {
            rect.max.x - knob_radius - 3.0
        } else {
            rect.min.x + knob_radius + 3.0
        };

        let knob_center = Pos2::new(x_pos, rect.center().y);
        let knob_color = if is_checked {
            theme.on_primary
        } else {
            theme.on_surface_variant
        };

        painter.circle_filled(knob_center, knob_radius, knob_color);
    }

    response
}
