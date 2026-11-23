//! Settings shell (category rail + page chrome) and the shared building blocks
//! used by every settings page.

pub mod advanced;
pub mod appearance;
pub mod audio;
pub mod general;
pub mod storage;
pub mod transcription;

use egui::{Color32, RichText, Ui};

use crate::app::Screen;
use crate::theme::ThemeColors;

/// Content column width of every settings page (matches Stitch `max-w-2xl`).
const PAGE_MAX_WIDTH: f32 = 672.0;

/// Inner 200px settings category navigation rail. Returns the newly picked page.
pub fn category_rail(ui: &mut Ui, current: Screen, theme: &ThemeColors) -> Option<Screen> {
    let categories: [(&str, &str, Screen); 6] = [
        (
            egui_phosphor::regular::SLIDERS_HORIZONTAL,
            "General",
            Screen::SettingsGeneral,
        ),
        (
            egui_phosphor::regular::HEADPHONES,
            "Audio",
            Screen::SettingsAudio,
        ),
        (
            egui_phosphor::regular::TRANSLATE,
            "Transcription",
            Screen::SettingsTranscription,
        ),
        (
            egui_phosphor::regular::HARD_DRIVE,
            "Storage",
            Screen::SettingsStorage,
        ),
        (
            egui_phosphor::regular::PALETTE,
            "Appearance",
            Screen::SettingsAppearance,
        ),
        (
            egui_phosphor::regular::TERMINAL,
            "Advanced",
            Screen::SettingsAdvanced,
        ),
    ];

    let mut picked = None;
    egui::SidePanel::left("settings_category_rail")
        .exact_width(200.0)
        .resizable(false)
        .frame(
            egui::Frame::NONE
                .fill(theme.sidebar)
                .stroke(egui::Stroke::new(1.0, theme.outline))
                .inner_margin(egui::Margin::same(12)),
        )
        .show_inside(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            for (icon, label, screen) in categories {
                let active = current == screen;
                let (rect, resp) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 34.0),
                    egui::Sense::click(),
                );
                let (bg, fg) = if active {
                    (theme.surface_elevated, theme.on_surface)
                } else if resp.hovered() {
                    (
                        Color32::from_rgba_unmultiplied(
                            theme.surface_elevated.r(),
                            theme.surface_elevated.g(),
                            theme.surface_elevated.b(),
                            140,
                        ),
                        theme.on_surface,
                    )
                } else {
                    (Color32::TRANSPARENT, theme.on_surface_variant)
                };
                let painter = ui.painter();
                painter.rect_filled(rect, egui::CornerRadius::same(6), bg);
                painter.text(
                    egui::pos2(rect.left() + 20.0, rect.center().y),
                    egui::Align2::CENTER_CENTER,
                    icon,
                    egui::FontId::proportional(16.0),
                    fg,
                );
                painter.text(
                    egui::pos2(rect.left() + 40.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    crate::i18n::tr(label),
                    egui::FontId::proportional(13.0),
                    fg,
                );
                if resp
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    picked = Some(screen);
                }
            }
        });
    picked
}

/// Page chrome: scrollable column with title + description, then `add`.
pub fn page(ui: &mut Ui, theme: &ThemeColors, title: &str, desc: &str, add: impl FnOnce(&mut Ui)) {
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(theme.background))
        .show_inside(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt(("settings_page", title))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::Frame::NONE
                        .inner_margin(egui::Margin::symmetric(24, 24))
                        .show(ui, |ui| {
                            ui.set_max_width(PAGE_MAX_WIDTH);
                            ui.spacing_mut().item_spacing.y = 24.0;
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 4.0;
                                ui.label(
                                    RichText::new(crate::i18n::tr(title))
                                        .size(22.0)
                                        .family(crate::theme::bold_family())
                                        .color(theme.on_surface),
                                );
                                ui.label(
                                    RichText::new(crate::i18n::tr(desc))
                                        .size(14.0)
                                        .color(theme.on_surface_variant),
                                );
                            });
                            ui.spacing_mut().item_spacing.y = 24.0;
                            add(ui);
                        });
                });
        });
}

pub fn section_title(ui: &mut Ui, theme: &ThemeColors, text: &str) {
    ui.label(
        RichText::new(crate::i18n::tr(text))
            .size(16.0)
            .family(crate::theme::bold_family())
            .color(theme.on_surface),
    );
}

/// Bordered surface card; children are spaced 20px apart.
pub fn card(ui: &mut Ui, theme: &ThemeColors, add: impl FnOnce(&mut Ui)) {
    egui::Frame::NONE
        .fill(theme.surface)
        .stroke(egui::Stroke::new(1.0, theme.outline))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 30.0;
            add(ui);
        });
}

/// Setting row: bold title + muted description on the left, control on the
/// right (vertically centred). `control_w` reserves room for the control.
pub fn row(
    ui: &mut Ui,
    theme: &ThemeColors,
    title: &str,
    desc: Option<&str>,
    control_w: f32,
    control: impl FnOnce(&mut Ui),
) {
    row_styled(ui, theme, title, desc, false, None, control_w, control);
}

#[allow(clippy::too_many_arguments)]
pub fn row_styled(
    ui: &mut Ui,
    theme: &ThemeColors,
    title: &str,
    desc: Option<&str>,
    mono_desc: bool,
    tag: Option<&str>,
    control_w: f32,
    control: impl FnOnce(&mut Ui),
) {
    let full_w = ui.available_width();
    let left_w = (full_w - control_w - 24.0).max(120.0);
    let origin = ui.cursor().min;
    let dimmed = tag.is_some();
    let (title_col, desc_col) = if dimmed {
        (theme.on_surface_variant, theme.outline)
    } else {
        (theme.on_surface, theme.on_surface_variant)
    };

    // Title-only rows are centred against a 34px control; others size to their text.
    let (min_h, layout) = if desc.is_none() {
        (34.0, egui::Layout::left_to_right(egui::Align::Center))
    } else {
        (0.0, egui::Layout::top_down(egui::Align::Min))
    };
    let left = ui.allocate_ui_with_layout(egui::vec2(left_w, min_h), layout, |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 2.0);
        let header = |ui: &mut Ui| {
            let title_text = RichText::new(crate::i18n::tr(title))
                .size(14.0)
                .color(title_col);
            ui.label(if desc.is_some() {
                title_text.family(crate::theme::bold_family())
            } else {
                title_text
            });
            if let Some(tag) = tag {
                pill(
                    ui,
                    tag,
                    theme.surface_elevated,
                    theme.on_surface_variant,
                    Some(theme.outline),
                );
            }
        };
        if desc.is_none() {
            header(ui);
        } else {
            ui.horizontal(header);
        }
        if let Some(d) = desc {
            let mut t = RichText::new(crate::i18n::tr(d)).color(desc_col);
            t = if mono_desc {
                t.font(egui::FontId::monospace(12.0))
            } else {
                t.size(13.0)
            };
            ui.add(egui::Label::new(t).wrap());
        }
    });
    let left_h = left.response.rect.height();
    let row_h = left_h.max(34.0);
    if row_h > left_h {
        ui.add_space(row_h - left_h);
    }
    let ctrl_rect = egui::Rect::from_min_size(origin, egui::vec2(full_w, row_h));
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(ctrl_rect)
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    control(&mut child);
}

/// Form dropdown (recessed fill); `mono` renders options in monospace.
pub fn select(
    ui: &mut Ui,
    theme: &ThemeColors,
    id: &str,
    index: &mut usize,
    options: &[&str],
    width: f32,
    mono: bool,
) {
    crate::components::select::dropdown(
        ui,
        theme,
        id,
        index,
        options,
        width,
        theme.background,
        mono,
    );
}

/// Monospace path input on the recessed background.
pub fn path_field(ui: &mut Ui, theme: &ThemeColors, text: &mut String, width: f32) {
    egui::Frame::NONE
        .fill(theme.background)
        .stroke(egui::Stroke::new(1.0, theme.outline))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(12, 0))
        .show(ui, |ui| {
            ui.set_height(34.0);
            ui.set_width((width - 24.0).max(40.0));
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.add(
                    egui::TextEdit::singleline(text)
                        .font(egui::FontId::monospace(12.5))
                        .text_color(theme.on_surface)
                        .desired_width(ui.available_width())
                        .frame(false),
                );
            });
        });
}

/// A labelled field: bold title, then `add` (e.g. a path field + Browse button).
pub fn field(ui: &mut Ui, theme: &ThemeColors, title: &str, add: impl FnOnce(&mut Ui)) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 8.0;
        ui.label(
            RichText::new(crate::i18n::tr(title))
                .size(14.0)
                .family(crate::theme::bold_family())
                .color(theme.on_surface),
        );
        add(ui);
    });
}

/// Small bordered pill (value display such as "Thai (TH)").
pub fn pill(ui: &mut Ui, text: &str, bg: Color32, fg: Color32, border: Option<Color32>) {
    egui::Frame::NONE
        .fill(bg)
        .stroke(match border {
            Some(c) => egui::Stroke::new(1.0, c),
            None => egui::Stroke::NONE,
        })
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(crate::i18n::tr(text))
                        .size(11.5)
                        .family(crate::theme::bold_family())
                        .color(fg),
                );
            });
        });
}
