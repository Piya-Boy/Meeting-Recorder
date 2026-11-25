//! Modal dialogs: Help, About and Delete-recording confirmation.

use crate::i18n::tr;
use egui::{Color32, Context, RichText, Ui};

use crate::app::{AppState, Modal};
use crate::components::button::{custom_button, ButtonVariant};
use crate::theme::{bold_family, ThemeColors};

const SHORTCUTS: [(&str, &str); 3] = [
    ("Start / Stop recording", "Ctrl + Shift + R"),
    ("Pause / Resume recording", "Ctrl + Shift + P"),
    ("Copy live transcript", "Ctrl + Shift + C"),
];

/// Draws whichever modal is open. Esc or a click on the scrim closes it.
pub fn draw(ctx: &Context, state: &mut AppState, theme: &ThemeColors) {
    let Some(modal) = state.open_modal.clone() else {
        return;
    };

    let frame = egui::Frame::NONE
        .fill(theme.surface)
        .stroke(egui::Stroke::new(1.0, theme.outline))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(24));

    let mut close = false;
    let response = egui::Modal::new(egui::Id::new("app_modal"))
        .backdrop_color(theme.scrim)
        .frame(frame)
        .show(ctx, |ui| {
            ui.set_width(448.0);
            ui.spacing_mut().item_spacing.y = 16.0;
            match &modal {
                Modal::Help => help_body(ui, theme, &mut close),
                Modal::About => about_body(ui, theme, &mut close),
                Modal::DeleteRecording { id, name, dir } => {
                    delete_body(ui, theme, state, *id, name, dir.clone(), &mut close)
                }
            }
        });

    if close || response.should_close() {
        state.open_modal = None;
    }
}

fn heading(ui: &mut Ui, theme: &ThemeColors, text: &str) {
    ui.label(
        RichText::new(text)
            .size(16.0)
            .family(bold_family())
            .color(theme.on_surface),
    );
}

fn footer(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        add(ui);
    });
}

fn help_body(ui: &mut Ui, theme: &ThemeColors, close: &mut bool) {
    heading(ui, theme, tr("Keyboard shortcuts & Help"));
    ui.spacing_mut().item_spacing.y = 12.0;
    for (action, keys) in SHORTCUTS {
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr(action)).size(13.0).color(theme.on_surface));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::Frame::NONE
                    .fill(theme.background)
                    .stroke(egui::Stroke::new(1.0, theme.outline))
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(10, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(keys)
                                    .font(egui::FontId::monospace(12.0))
                                    .family(crate::theme::mono_bold_family())
                                    .color(theme.on_surface),
                            );
                        });
                    });
            });
        });
    }
    ui.add_space(4.0);
    footer(ui, |ui| {
        if custom_button(ui, theme, None, "Close", ButtonVariant::Secondary).clicked() {
            *close = true;
        }
    });
}

fn about_body(ui: &mut Ui, theme: &ThemeColors, close: &mut bool) {
    heading(ui, theme, tr("About Meeting Recorder"));
    ui.label(
        RichText::new(format!(
            "Meeting Recorder v{} (x86_64 Windows 11)",
            crate::update::VERSION
        ))
        .font(egui::FontId::monospace(12.0))
        .color(theme.on_surface),
    );
    ui.add(
        egui::Label::new(
            RichText::new(
                "Local-first dual-track audio capture and real-time Thai transcription powered by \
                 Rust, egui/eframe, Windows WASAPI, OpenThai ASR and ONNX Runtime.",
            )
            .size(13.0)
            .color(theme.on_surface_variant),
        )
        .wrap(),
    );
    ui.add(
        egui::Label::new(
            RichText::new(tr(
                "Licences of the libraries and of the speech model: THIRD_PARTY_NOTICES.md in the project folder.",
            ))
            .size(12.0)
            .color(theme.on_surface_variant),
        )
        .wrap(),
    );
    ui.add_space(4.0);
    footer(ui, |ui| {
        if custom_button(ui, theme, None, "Close", ButtonVariant::Secondary).clicked() {
            *close = true;
        }
    });
}

fn delete_body(
    ui: &mut Ui,
    theme: &ThemeColors,
    state: &mut AppState,
    id: usize,
    name: &str,
    dir: Option<std::path::PathBuf>,
    close: &mut bool,
) {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        let (icon_rect, _) = ui.allocate_exact_size(egui::Vec2::splat(36.0), egui::Sense::hover());
        ui.painter().rect_filled(
            icon_rect,
            egui::CornerRadius::same(6),
            Color32::from_rgba_unmultiplied(theme.error.r(), theme.error.g(), theme.error.b(), 38),
        );
        ui.painter().text(
            icon_rect.center(),
            egui::Align2::CENTER_CENTER,
            egui_phosphor::regular::TRASH,
            egui::FontId::proportional(18.0),
            theme.error,
        );
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            heading(ui, theme, tr("Delete recording?"));
            let mut job = egui::text::LayoutJob::default();
            job.wrap.max_width = ui.available_width();
            let normal = egui::TextFormat {
                font_id: egui::FontId::proportional(13.0),
                color: theme.on_surface_variant,
                ..Default::default()
            };
            let strong = egui::TextFormat {
                font_id: egui::FontId::new(13.0, bold_family()),
                color: theme.on_surface,
                ..Default::default()
            };
            job.append(tr("This will permanently delete "), 0.0, normal.clone());
            job.append(name, 0.0, strong);
            job.append(
                tr(" and its dual-track WAV file from disk. This action cannot be undone."),
                0.0,
                normal,
            );
            ui.label(job);
        });
    });
    ui.add_space(4.0);
    footer(ui, |ui| {
        // right-to-left: primary action first
        if custom_button(ui, theme, None, "Delete", ButtonVariant::Danger).clicked() {
            state.apply_delete(id, dir);
            *close = true;
        }
        if custom_button(ui, theme, None, "Cancel", ButtonVariant::Secondary).clicked() {
            *close = true;
        }
    });
}
