use crate::theme::ThemeColors;
use egui::{vec2, CornerRadius, Pos2, Rect, Stroke, Ui};

pub fn audio_level_meter(
    ui: &mut Ui,
    theme: &ThemeColors,
    channel_name: &str,
    level_db: f32, // e.g. -14.0
    active: bool,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 0.0);

        // Label (SYS / MIC)
        let mono_font = egui::FontId::monospace(12.0);
        ui.add_sized(
            vec2(32.0, 18.0),
            egui::Label::new(
                egui::RichText::new(channel_name)
                    .font(mono_font.clone())
                    .color(theme.on_surface_variant),
            ),
        );

        // Meter Bar Track
        let bar_height = 8.0;
        let (rect, _) = ui.allocate_exact_size(
            vec2((ui.available_width() - 56.0).max(0.0), bar_height),
            egui::Sense::hover(),
        );

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            // Track background
            painter.rect(
                rect,
                CornerRadius::same(2),
                theme.background,
                Stroke::new(1.0, theme.outline_variant),
                egui::StrokeKind::Inside,
            );

            if active {
                // Map dB (-60 to 0) to 0.0..1.0
                let fraction = ((level_db + 60.0) / 60.0).clamp(0.0, 1.0);
                let filled_width = rect.width() * fraction;

                let green_ratio = 0.75;
                let green_width = (filled_width).min(rect.width() * green_ratio);

                if green_width > 0.0 {
                    let green_rect = Rect::from_min_size(
                        rect.min + vec2(1.0, 1.0),
                        vec2((green_width - 2.0).max(1.0), bar_height - 2.0),
                    );
                    painter.rect_filled(green_rect, CornerRadius::same(2), theme.success);
                }

                if filled_width > rect.width() * green_ratio {
                    let yellow_width = filled_width - (rect.width() * green_ratio);
                    let yellow_rect = Rect::from_min_size(
                        Pos2::new(rect.min.x + 1.0 + green_width, rect.min.y + 1.0),
                        vec2((yellow_width - 2.0).max(1.0), bar_height - 2.0),
                    );
                    painter.rect_filled(yellow_rect, CornerRadius::same(2), theme.warning);
                }
            }
        }

        // dB Readout
        let db_text = if active {
            format!("{:.0} dB", level_db)
        } else {
            "-∞ dB".to_owned()
        };

        ui.add_sized(
            vec2(48.0, 18.0),
            egui::Label::new(
                egui::RichText::new(db_text)
                    .font(mono_font)
                    .color(theme.on_surface_variant),
            ),
        );
    });
}
