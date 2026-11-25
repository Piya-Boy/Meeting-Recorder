use egui::{Color32, Ui};

use super::{card, page, row, select};
use crate::theme::{ThemeColors, ThemeMode};

const FONT_SCALES: [&str; 4] = ["90%", "100% (Default)", "110%", "125%"];
const FONT_SCALE_VALUES: [f32; 4] = [0.9, 1.0, 1.1, 1.25];

/// Selectable accent colours: (name, swatch colour).
pub const ACCENTS: [(&str, Color32); 3] = [
    ("Blue", Color32::from_rgb(0x3B, 0x82, 0xF6)),
    ("Green", Color32::from_rgb(0x16, 0xA3, 0x4A)),
    ("Amber", Color32::from_rgb(0xD9, 0x77, 0x06)),
];

pub struct SettingsAppearanceScreen {
    pub font_scale_idx: usize,
    pub accent_idx: usize,
}

impl Default for SettingsAppearanceScreen {
    fn default() -> Self {
        Self {
            font_scale_idx: 1,
            accent_idx: 0,
        }
    }
}

impl SettingsAppearanceScreen {
    pub fn zoom_factor(&self) -> f32 {
        FONT_SCALE_VALUES[self.font_scale_idx.min(FONT_SCALE_VALUES.len() - 1)]
    }
}

pub fn draw_settings_appearance(
    ui: &mut Ui,
    screen: &mut SettingsAppearanceScreen,
    theme: &ThemeColors,
    theme_mode: &mut ThemeMode,
) {
    page(
        ui,
        theme,
        "Appearance",
        "Select a dark, light or Windows system theme, an accent color and the UI font scale.",
        |ui| {
            // Theme picker: three equal cards.
            let gap = 12.0;
            let card_w = ((ui.available_width() - gap * 2.0) / 3.0).floor();
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for (mode, label, tag) in [
                    (ThemeMode::Dark, "Dark theme", "Default"),
                    (ThemeMode::Light, "Light theme", "Preview"),
                    (ThemeMode::System, "System theme", "Windows"),
                ] {
                    if theme_card(ui, theme, card_w, mode, label, tag, *theme_mode == mode)
                        .clicked()
                    {
                        *theme_mode = mode;
                    }
                }
            });

            card(ui, theme, |ui| {
                row(
                    ui,
                    theme,
                    "Accent color",
                    Some("Primary interactive highlight across buttons, toggles, and active navigation."),
                    110.0,
                    |ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        // right-to-left: add last swatch first
                        for i in (0..ACCENTS.len()).rev() {
                            if swatch(ui, theme, ACCENTS[i], screen.accent_idx == i).clicked() {
                                screen.accent_idx = i;
                            }
                        }
                    },
                );
                row(
                    ui,
                    theme,
                    "Font scaling",
                    Some("Adjust UI and Thai transcript legibility scale."),
                    180.0,
                    |ui| {
                        select(
                            ui,
                            theme,
                            "font_scale",
                            &mut screen.font_scale_idx,
                            &FONT_SCALES,
                            160.0,
                            true,
                        )
                    },
                );
            });
        },
    );
}

fn swatch(
    ui: &mut Ui,
    theme: &ThemeColors,
    accent: (&str, Color32),
    selected: bool,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::Vec2::splat(28.0), egui::Sense::click());
    let resp = resp
        .on_hover_text(crate::i18n::tr(accent.0))
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let painter = ui.painter();
    painter.circle_filled(rect.center(), 10.0, accent.1);
    if selected {
        painter.circle_stroke(
            rect.center(),
            13.0,
            egui::Stroke::new(2.0, theme.on_surface),
        );
    }
    resp
}

fn theme_card(
    ui: &mut Ui,
    theme: &ThemeColors,
    width: f32,
    mode: ThemeMode,
    label: &str,
    tag: &str,
    selected: bool,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, 117.0), egui::Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let painter = ui.painter();
    let radius = egui::CornerRadius::same(8);
    let hovered = resp.hovered();
    painter.rect_filled(
        rect,
        radius,
        if mode == ThemeMode::Light {
            ThemeColors::LIGHT.background
        } else {
            theme.surface
        },
    );
    painter.rect_stroke(
        rect,
        radius,
        if selected {
            egui::Stroke::new(2.0, theme.brand_blue)
        } else if hovered {
            egui::Stroke::new(1.0, theme.on_surface_variant)
        } else {
            egui::Stroke::new(1.0, theme.outline)
        },
        egui::StrokeKind::Inside,
    );

    // Mini window preview
    let preview = egui::Rect::from_min_max(
        rect.min + egui::vec2(10.0, 10.0),
        egui::pos2(rect.max.x - 10.0, rect.min.y + 78.0),
    );
    let small = egui::CornerRadius::same(4);
    let mini = |painter: &egui::Painter, area: egui::Rect, c: &ThemeColors| {
        painter.rect_filled(area, small, c.background);
        let strip = egui::Rect::from_min_size(area.min, egui::vec2(24.0, area.height()));
        painter.rect_filled(
            strip,
            egui::CornerRadius {
                nw: 4,
                sw: 4,
                ne: 0,
                se: 0,
            },
            c.sidebar,
        );
        painter.rect_filled(
            egui::Rect::from_min_size(area.min + egui::vec2(34.0, 10.0), egui::vec2(36.0, 6.0)),
            egui::CornerRadius::same(3),
            c.primary,
        );
        painter.rect_filled(
            egui::Rect::from_min_size(area.min + egui::vec2(34.0, 21.0), egui::vec2(48.0, 4.0)),
            egui::CornerRadius::same(2),
            c.on_surface_variant,
        );
    };
    match mode {
        ThemeMode::Dark => mini(painter, preview, &ThemeColors::DARK),
        ThemeMode::Light => {
            mini(painter, preview, &ThemeColors::LIGHT);
            painter.rect_stroke(
                preview,
                small,
                egui::Stroke::new(1.0, ThemeColors::LIGHT.outline),
                egui::StrokeKind::Inside,
            );
        }
        ThemeMode::System => {
            let half = preview.width() / 2.0;
            let left = egui::Rect::from_min_size(preview.min, egui::vec2(half, preview.height()));
            let right = egui::Rect::from_min_max(
                egui::pos2(preview.min.x + half, preview.min.y),
                preview.max,
            );
            painter.rect_filled(
                left,
                egui::CornerRadius {
                    nw: 4,
                    sw: 4,
                    ne: 0,
                    se: 0,
                },
                ThemeColors::DARK.background,
            );
            painter.rect_filled(
                right,
                egui::CornerRadius {
                    nw: 0,
                    sw: 0,
                    ne: 4,
                    se: 4,
                },
                ThemeColors::LIGHT.background,
            );
        }
    }

    let text_col = if mode == ThemeMode::Light {
        ThemeColors::LIGHT.on_surface
    } else {
        theme.on_surface
    };
    painter.text(
        egui::pos2(rect.left() + 12.0, rect.bottom() - 18.0),
        egui::Align2::LEFT_CENTER,
        crate::i18n::tr(label),
        egui::FontId::proportional(13.0),
        text_col,
    );
    let tag_col = if selected {
        theme.brand_blue
    } else if mode == ThemeMode::Light {
        ThemeColors::LIGHT.on_surface_variant
    } else {
        theme.on_surface_variant
    };
    painter.text(
        egui::pos2(rect.right() - 12.0, rect.bottom() - 18.0),
        egui::Align2::RIGHT_CENTER,
        crate::i18n::tr(tag),
        egui::FontId::proportional(11.5),
        tag_col,
    );
    resp
}
