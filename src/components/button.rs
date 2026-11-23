use crate::theme::ThemeColors;
use egui::{vec2, Color32, CornerRadius, Pos2, Response, Stroke, Ui};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    Primary,
    Secondary,
    Danger,
}

/// Standard 36px button (13px medium label) — Stitch `px-3 py-2` + 1px border.
pub fn custom_button(
    ui: &mut Ui,
    theme: &ThemeColors,
    icon: Option<&str>,
    text: &str,
    variant: ButtonVariant,
) -> Response {
    sized_button(
        ui,
        theme,
        icon,
        text,
        variant,
        36.0,
        egui::FontId::new(13.0, crate::theme::medium_family()),
    )
}

/// Compact 26px button (11px medium label) for table rows and toolbars.
pub fn compact_button(
    ui: &mut Ui,
    theme: &ThemeColors,
    icon: Option<&str>,
    text: &str,
    variant: ButtonVariant,
) -> Response {
    sized_button(
        ui,
        theme,
        icon,
        text,
        variant,
        26.0,
        egui::FontId::new(11.0, crate::theme::medium_family()),
    )
}

/// Compact button with a monospace label (export format chips: TXT / SRT / JSON).
pub fn mono_chip_button(ui: &mut Ui, theme: &ThemeColors, text: &str) -> Response {
    sized_button(
        ui,
        theme,
        None,
        text,
        ButtonVariant::Secondary,
        26.0,
        egui::FontId::monospace(11.5),
    )
}

/// Standard 36px button stretched to `width` (transport-control grid).
pub fn block_button(
    ui: &mut Ui,
    theme: &ThemeColors,
    icon: Option<&str>,
    text: &str,
    variant: ButtonVariant,
    width: f32,
) -> Response {
    button_impl(
        ui,
        theme,
        icon,
        text,
        variant,
        36.0,
        egui::FontId::new(13.0, crate::theme::medium_family()),
        Some(width),
    )
}

pub fn sized_button(
    ui: &mut Ui,
    theme: &ThemeColors,
    icon: Option<&str>,
    text: &str,
    variant: ButtonVariant,
    height: f32,
    font_id: egui::FontId,
) -> Response {
    button_impl(ui, theme, icon, text, variant, height, font_id, None)
}

#[allow(clippy::too_many_arguments)]
fn button_impl(
    ui: &mut Ui,
    theme: &ThemeColors,
    icon: Option<&str>,
    text: &str,
    variant: ButtonVariant,
    height: f32,
    font_id: egui::FontId,
    width: Option<f32>,
) -> Response {
    let text = crate::i18n::tr(text);
    let label_text = if let Some(ic) = icon {
        if text.is_empty() {
            ic.to_owned()
        } else {
            format!("{}  {}", ic, text)
        }
    } else {
        text.to_owned()
    };

    let galley = ui
        .painter()
        .layout_no_wrap(label_text, font_id, Color32::PLACEHOLDER);
    let padding = vec2(if height < 30.0 { 10.0 } else { 12.0 }, 7.0);
    let size = vec2(
        width.unwrap_or(galley.size().x + padding.x * 2.0).max(0.0),
        height.max(galley.size().y + padding.y * 2.0),
    );

    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);

    if ui.is_rect_visible(rect) {
        let is_hovered = response.hovered();
        let is_active = response.is_pointer_button_down_on();

        // Stitch: `hover:opacity-90` — no border/colour change, whole control at 90 %.
        let (bg, fg, stroke) = match variant {
            ButtonVariant::Primary => (theme.primary, theme.on_primary, Stroke::NONE),
            ButtonVariant::Secondary => (
                theme.surface_elevated,
                theme.on_surface,
                Stroke::new(1.0, theme.outline),
            ),
            ButtonVariant::Danger => (theme.error, theme.on_error, Stroke::NONE),
        };
        let (bg, fg, stroke) = if is_hovered || is_active {
            (
                bg.gamma_multiply(0.9),
                fg.gamma_multiply(0.9),
                Stroke::new(stroke.width, stroke.color.gamma_multiply(0.9)),
            )
        } else {
            (bg, fg, stroke)
        };

        let painter = ui.painter();
        painter.rect(
            rect,
            CornerRadius::same(6),
            bg,
            stroke,
            egui::StrokeKind::Inside,
        );

        let text_pos = Pos2::new(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        );
        painter.galley(text_pos, galley, fg);
    }

    response
}

pub fn icon_button(ui: &mut Ui, theme: &ThemeColors, icon: &str, size: f32) -> Response {
    let font_id = egui::FontId::proportional(14.0);
    let galley = ui
        .painter()
        .layout_no_wrap(icon.to_owned(), font_id, Color32::PLACEHOLDER);
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), egui::Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);

    if ui.is_rect_visible(rect) {
        let is_hovered = response.hovered();
        let is_active = response.is_pointer_button_down_on();

        let fade = if is_hovered || is_active { 0.9 } else { 1.0 };
        let bg = theme.surface_elevated.gamma_multiply(fade);
        let stroke = Stroke::new(1.0, theme.outline.gamma_multiply(fade));
        let fg = theme.on_surface_variant.gamma_multiply(fade);

        let painter = ui.painter();
        painter.rect(
            rect,
            CornerRadius::same(6),
            bg,
            stroke,
            egui::StrokeKind::Inside,
        );

        let text_pos = Pos2::new(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        );
        painter.galley(text_pos, galley, fg);
    }

    response
}

/// Square icon button with a variant (primary = filled accent, secondary = bordered).
pub fn sized_icon_button(
    ui: &mut Ui,
    theme: &ThemeColors,
    icon: &str,
    size: f32,
    variant: ButtonVariant,
) -> Response {
    if variant == ButtonVariant::Secondary {
        return icon_button(ui, theme, icon, size);
    }
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), egui::Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    if ui.is_rect_visible(rect) {
        let bg = if response.hovered() {
            theme.primary.gamma_multiply(0.9)
        } else {
            theme.primary
        };
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(6), bg);
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(if size > 30.0 { 16.0 } else { 14.0 }),
            theme.on_primary,
        );
    }
    response
}
