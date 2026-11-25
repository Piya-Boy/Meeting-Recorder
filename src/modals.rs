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
            ui.set_width(match modal {
                // The licence texts are laid out for `LICENCE_COLUMNS` monospace characters.
                Modal::Licenses => (ctx.screen_rect().width() - 96.0).min(680.0),
                _ => 448.0,
            });
            ui.spacing_mut().item_spacing.y = 16.0;
            match &modal {
                Modal::Help => help_body(ui, theme, &mut close),
                Modal::About => about_body(ui, theme, &mut close),
                Modal::DeleteRecording { id, name, dir } => {
                    delete_body(ui, theme, state, *id, name, dir.clone(), &mut close)
                }
                Modal::QuitWhileRecording => quit_body(ui, theme, state, &mut close),
                Modal::Licenses => licences_body(ui, theme, &mut close),
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
            "Meeting Recorder v{} (x86_64 Windows)",
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
                "Licences of the libraries, fonts and speech model: Settings > Advanced > Open-source licenses.",
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

/// The widest line of the licence viewer, in characters.
const LICENCE_COLUMNS: usize = 90;

/// Breaks `line` at spaces so that no piece is longer than `max` characters. A word that is
/// longer than that is cut.
fn wrap_line(line: &str, max: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    // `limit` is the byte offset of the first character that does not fit.
    while let Some((limit, next)) = rest.char_indices().nth(max) {
        let head = &rest[..limit];
        let cut = if next == ' ' {
            limit
        } else {
            head.rfind(' ')
                .filter(|&i| !head[..i].trim().is_empty())
                .unwrap_or(limit)
        };
        out.push(rest[..cut].trim_end().to_owned());
        rest = rest[cut..].trim_start_matches(' ');
    }
    if !rest.is_empty() || out.is_empty() {
        out.push(rest.to_owned());
    }
    out
}

/// The notices and licence texts, built into the program, as viewer lines.
fn licence_lines() -> &'static [String] {
    static LINES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    LINES.get_or_init(|| {
        [
            include_str!("../THIRD_PARTY_NOTICES.md"),
            include_str!("../THIRD_PARTY_LICENSES.txt"),
        ]
        .iter()
        .flat_map(|text| text.lines().chain(std::iter::once("")))
        .flat_map(|line| wrap_line(&line.replace('\t', "    "), LICENCE_COLUMNS))
        .collect()
    })
}

fn licences_body(ui: &mut Ui, theme: &ThemeColors, close: &mut bool) {
    heading(ui, theme, tr("Open-source licenses"));
    let font = egui::FontId::monospace(11.0);
    let row_height = ui.fonts(|f| f.row_height(&font));
    let height = (ui.ctx().screen_rect().height() - 240.0).clamp(200.0, 520.0);
    let lines = licence_lines();
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        // Only the visible rows are laid out: there are thousands of them.
        egui::ScrollArea::vertical()
            .id_salt("licences")
            .max_height(height)
            .auto_shrink(false)
            .show_rows(ui, row_height, lines.len(), |ui, visible| {
                for line in &lines[visible] {
                    ui.add(
                        egui::Label::new(
                            RichText::new(line)
                                .font(font.clone())
                                .color(theme.on_surface),
                        )
                        .wrap_mode(egui::TextWrapMode::Extend),
                    );
                }
            });
    });
    footer(ui, |ui| {
        if custom_button(ui, theme, None, "Close", ButtonVariant::Secondary).clicked() {
            *close = true;
        }
    });
}

/// Esc, the scrim and "Keep recording" all leave the recording running.
fn quit_body(ui: &mut Ui, theme: &ThemeColors, state: &mut AppState, close: &mut bool) {
    heading(ui, theme, tr("Stop recording and quit?"));
    ui.add(
        egui::Label::new(
            RichText::new(tr(
                "The recording stops here and is saved, but not transcribed. To transcribe it, open the app again and choose Transcribe on the Recordings page.",
            ))
            .size(13.0)
            .color(theme.on_surface_variant),
        )
        .wrap(),
    );
    ui.add_space(4.0);
    footer(ui, |ui| {
        // right-to-left: primary action first
        if custom_button(ui, theme, None, "Stop and quit", ButtonVariant::Danger).clicked() {
            state.quit_while_recording(ui.ctx());
            *close = true;
        }
        if custom_button(ui, theme, None, "Keep recording", ButtonVariant::Secondary).clicked() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_lines_are_wrapped_for_the_viewer() {
        assert_eq!(wrap_line("", 10), vec![""]);
        assert_eq!(wrap_line("short", 10), vec!["short"]);
        assert_eq!(wrap_line("  indented", 10), vec!["  indented"]);
        assert_eq!(wrap_line("aaaa bbbb cccc", 10), vec!["aaaa bbbb", "cccc"]);
        // A word longer than the line is cut, between characters and never inside one.
        assert_eq!(wrap_line("abcdefghijkl", 5), vec!["abcde", "fghij", "kl"]);
        assert_eq!(wrap_line("©©©©©©", 4), vec!["©©©©", "©©"]);
    }

    #[test]
    fn the_viewer_holds_every_licence_that_ships() {
        let lines = licence_lines();
        let all = lines.join("\n");
        for wanted in [
            "# Third-party notices",
            "PoodPim",
            "OpenMDW",
            "Font: Noto Sans Thai (SIL Open Font License 1.1)",
            "ONNX Runtime 1.31.0 (MIT)",
            "- egui 0.31.1",
            "Emil Ernerfeldt",
            "- windows-sys",
        ] {
            assert!(all.contains(wanted), "{wanted} is missing");
        }
        assert!(
            !all.contains("<copyright holders>"),
            "a licence still has the template line"
        );
        let widest = lines.iter().map(|l| l.chars().count()).max();
        assert!(widest <= Some(LICENCE_COLUMNS), "{widest:?}");
    }
}
