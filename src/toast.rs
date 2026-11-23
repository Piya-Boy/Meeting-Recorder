//! Transient bottom-right notification, stored in egui's temp memory so any
//! screen can raise one with only a `&Context`.

use egui::{Align2, Context, Id, RichText};

use crate::theme::ThemeColors;

const LIFETIME_SECS: f64 = 2.5;

fn toast_id() -> Id {
    Id::new("app_toast")
}

pub fn show(ctx: &Context, text: impl Into<String>) {
    let until = ctx.input(|i| i.time) + LIFETIME_SECS;
    ctx.data_mut(|d| d.insert_temp(toast_id(), (text.into(), until)));
    ctx.request_repaint();
}

pub fn draw(ctx: &Context, theme: &ThemeColors) {
    let Some((text, until)) = ctx.data(|d| d.get_temp::<(String, f64)>(toast_id())) else {
        return;
    };
    let now = ctx.input(|i| i.time);
    if now > until {
        ctx.data_mut(|d| d.remove::<(String, f64)>(toast_id()));
        return;
    }
    egui::Area::new(Id::new("app_toast_area"))
        .anchor(Align2::RIGHT_BOTTOM, [-24.0, -24.0])
        .order(egui::Order::Tooltip)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(theme.surface_elevated)
                .stroke(egui::Stroke::new(1.0, theme.outline))
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(14, 10))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(egui_phosphor::regular::CHECK_CIRCLE)
                                .size(16.0)
                                .color(theme.success_text),
                        );
                        ui.label(
                            RichText::new(crate::i18n::tr(&text))
                                .size(13.0)
                                .color(theme.on_surface),
                        );
                    });
                });
        });
    ctx.request_repaint_after(std::time::Duration::from_millis(100));
}
