//! App shell: left navigation sidebar and top application bar.

use crate::i18n::tr;
use egui::{Color32, RichText, Ui};

use crate::app::{AppState, Modal, Screen};
use crate::screens::recorder::{state_badge_style, RecorderState};
use crate::theme::{ThemeColors, ThemeMode};

/// Full-width, left-aligned navigation row (icon + label) with hover/active states.
fn sidebar_item(
    ui: &mut Ui,
    theme: &ThemeColors,
    icon: &str,
    label: &str,
    active: bool,
    collapsed: bool,
) -> egui::Response {
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 34.0), egui::Sense::click());
    let (bg, fg) = if active {
        (theme.primary_container, theme.on_primary_container)
    } else if resp.hovered() {
        (theme.surface_elevated, theme.on_surface)
    } else {
        (Color32::TRANSPARENT, theme.on_surface_variant)
    };
    let painter = ui.painter();
    painter.rect_filled(rect, egui::CornerRadius::same(6), bg);
    let icon_x = if collapsed {
        rect.center().x
    } else {
        rect.left() + 20.0
    };
    painter.text(
        egui::pos2(icon_x, rect.center().y),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(16.0),
        fg,
    );
    if !collapsed {
        painter.text(
            egui::pos2(rect.left() + 40.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(13.0),
            fg,
        );
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(label)
}

pub fn draw_sidebar(ui: &mut Ui, state: &mut AppState, theme: &ThemeColors) {
    let collapsed = state.sidebar_collapsed;

    // ── Bottom block (Help / About / version) ──────────────────────────────
    egui::TopBottomPanel::bottom("sidebar_bottom")
        .resizable(false)
        .frame(
            egui::Frame::NONE
                .stroke(egui::Stroke::new(1.0, theme.outline_variant))
                .inner_margin(egui::Margin::same(8)),
        )
        .show_inside(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            if sidebar_item(
                ui,
                theme,
                egui_phosphor::regular::QUESTION,
                tr("Help"),
                false,
                collapsed,
            )
            .clicked()
            {
                state.open_modal = Some(Modal::Help);
            }
            if sidebar_item(
                ui,
                theme,
                egui_phosphor::regular::INFO,
                tr("About"),
                false,
                collapsed,
            )
            .clicked()
            {
                state.open_modal = Some(Modal::About);
            }
            if !collapsed {
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("v0.4.2")
                            .font(egui::FontId::monospace(12.0))
                            .color(theme.on_surface_variant),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new("Windows 11")
                                .size(11.0)
                                .color(theme.on_surface_variant),
                        );
                    });
                });
            }
        });

    // ── Brand header ───────────────────────────────────────────────────────
    egui::TopBottomPanel::top("sidebar_brand")
        .exact_height(48.0)
        .resizable(false)
        .frame(
            egui::Frame::NONE
                .stroke(egui::Stroke::new(1.0, theme.outline_variant))
                .inner_margin(egui::Margin::symmetric(12, 0)),
        )
        .show_inside(ui, |ui| {
            window_drag_region(
                ui,
                ui.max_rect().expand2(egui::vec2(12.0, 0.0)),
                "sidebar_drag",
            );
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                // Only clickable (and so keyboard-focusable) when it expands the sidebar.
                let logo_sense = if collapsed {
                    egui::Sense::click()
                } else {
                    egui::Sense::hover()
                };
                let (logo, logo_resp) = ui.allocate_exact_size(egui::Vec2::splat(28.0), logo_sense);
                if !crate::branding::paint_logo(ui, logo) {
                    ui.painter()
                        .rect_filled(logo, egui::CornerRadius::same(6), theme.primary);
                    ui.painter().text(
                        logo.center(),
                        egui::Align2::CENTER_CENTER,
                        egui_phosphor::regular::MICROPHONE,
                        egui::FontId::proportional(16.0),
                        theme.on_primary,
                    );
                }
                if collapsed {
                    if logo_resp.on_hover_text(tr("Expand sidebar")).clicked() {
                        state.sidebar_collapsed = false;
                    }
                } else {
                    ui.label(
                        RichText::new("Meeting Recorder")
                            .size(14.0)
                            .family(crate::theme::bold_family())
                            .color(theme.on_surface),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let btn = ui
                            .add(
                                egui::Button::new(
                                    RichText::new(egui_phosphor::regular::SIDEBAR_SIMPLE)
                                        .size(16.0)
                                        .color(theme.on_surface_variant),
                                )
                                .fill(Color32::TRANSPARENT)
                                .stroke(egui::Stroke::NONE),
                            )
                            .on_hover_text(tr("Collapse sidebar"));
                        if btn.clicked() {
                            state.sidebar_collapsed = true;
                        }
                    });
                }
            });
        });

    // ── Primary navigation ─────────────────────────────────────────────────
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::same(8)))
        .show_inside(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            let items: [(&str, &str, Screen); 3] = [
                (
                    egui_phosphor::regular::BROADCAST,
                    tr("Recorder"),
                    Screen::Recorder,
                ),
                (
                    egui_phosphor::regular::DISC,
                    tr("Recordings"),
                    Screen::Recordings,
                ),
                (
                    egui_phosphor::regular::FILE_TEXT,
                    tr("Transcripts"),
                    Screen::Transcripts,
                ),
            ];
            for (icon, label, screen) in items {
                let active = state.current_screen == screen
                    || (screen == Screen::Transcripts
                        && state.current_screen == Screen::TranscriptDetail);
                if sidebar_item(ui, theme, icon, label, active, collapsed).clicked() {
                    state.current_screen = screen;
                }
            }
            let settings_active = state.current_screen.is_settings();
            let clicked = sidebar_item(
                ui,
                theme,
                egui_phosphor::regular::GEAR,
                tr("Settings"),
                settings_active,
                collapsed,
            )
            .clicked();
            if clicked && !settings_active {
                state.current_screen = Screen::SettingsGeneral;
            }
        });
}

fn recorder_status_badge(ui: &mut Ui, state: &AppState, theme: &ThemeColors) {
    let (bg, fg, border) = state_badge_style(theme, state.recorder.state);
    egui::Frame::NONE
        .fill(bg)
        .stroke(border)
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if matches!(
                    state.recorder.state,
                    RecorderState::Idle | RecorderState::Saved
                ) {
                    let (dot, _) =
                        ui.allocate_exact_size(egui::Vec2::splat(8.0), egui::Sense::hover());
                    ui.painter().circle_filled(dot.center(), 3.5, theme.success);
                } else {
                    ui.label(
                        RichText::new(egui_phosphor::regular::RECORD)
                            .size(13.0)
                            .color(fg),
                    );
                }
                ui.label(
                    RichText::new(state.recorder.global_label())
                        .size(11.0)
                        .color(fg),
                );
            });
        });
}

pub fn draw_topbar(ui: &mut Ui, state: &mut AppState, theme: &ThemeColors) {
    if state.current_screen == Screen::TranscriptDetail {
        draw_detail_topbar(ui, state, theme);
        return;
    }
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        ui.label(
            RichText::new(state.current_screen.title())
                .size(16.0)
                .family(crate::theme::bold_family())
                .color(theme.on_surface),
        );
        if let Some(sub) = state.current_screen.subtitle() {
            ui.label(RichText::new("/").size(13.0).color(theme.outline));
            ui.label(
                RichText::new(sub)
                    .size(13.0)
                    .color(theme.on_surface_variant),
            );
        }
        if !state.current_screen.is_settings() {
            ui.label(RichText::new("|").size(14.0).color(theme.outline));
            recorder_status_badge(ui, state, theme);
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Theme toggle
            let theme_icon = if state.theme_mode == ThemeMode::Light {
                egui_phosphor::regular::MOON
            } else {
                egui_phosphor::regular::SUN
            };
            let toggle = ui
                .add(
                    egui::Button::new(
                        RichText::new(theme_icon)
                            .size(16.0)
                            .color(theme.on_surface_variant),
                    )
                    .fill(theme.surface_elevated)
                    .stroke(egui::Stroke::new(1.0, theme.outline))
                    .corner_radius(egui::CornerRadius::same(6))
                    .min_size(egui::Vec2::splat(32.0)),
                )
                .on_hover_text(tr("Switch Light / Dark theme"))
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if toggle.clicked() {
                state.theme_mode = if state.theme_mode == ThemeMode::Light {
                    ThemeMode::Dark
                } else {
                    ThemeMode::Light
                };
            }

            if state.current_screen.is_settings() {
                recorder_status_badge(ui, state, theme);
            }

            // Demo control: toggle the recordings empty state (sample data only)
            if state.current_screen == Screen::Recordings && !state.recordings.live {
                let label = if state.recordings.show_empty {
                    tr("Show recordings list")
                } else {
                    tr("Preview empty state")
                };
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(label)
                                .size(11.0)
                                .family(crate::theme::bold_family())
                                .color(theme.on_surface_variant),
                        )
                        .fill(theme.surface_elevated)
                        .stroke(egui::Stroke::new(1.0, theme.outline))
                        .corner_radius(egui::CornerRadius::same(6))
                        .min_size(egui::vec2(0.0, 32.0)),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    state.recordings.show_empty = !state.recordings.show_empty;
                }
            }

            // Recorder state switcher (segmented control) — Recorder screen only
            if state.current_screen == Screen::Recorder {
                egui::Frame::NONE
                    .fill(theme.background)
                    .stroke(egui::Stroke::new(1.0, theme.outline))
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::same(2))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            // `horizontal` follows the parent's right-to-left flow,
                            // so list the options in reverse visual order.
                            for (rs, label) in [
                                (RecorderState::Error, tr("Error")),
                                (RecorderState::Processing, tr("Processing")),
                                (RecorderState::Paused, tr("Paused")),
                                (RecorderState::Recording, tr("Recording")),
                                (RecorderState::Idle, tr("Idle")),
                            ] {
                                let active = state.recorder.state == rs;
                                let (bg, fg) = if active {
                                    (theme.surface_elevated, theme.on_surface)
                                } else {
                                    (Color32::TRANSPARENT, theme.on_surface_variant)
                                };
                                let b = ui.add(
                                    egui::Button::new(RichText::new(label).size(11.0).color(fg))
                                        .fill(bg)
                                        .stroke(egui::Stroke::NONE)
                                        .corner_radius(egui::CornerRadius::same(4))
                                        .min_size(egui::vec2(0.0, 24.0)),
                                );
                                if b.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                    state.recorder.state = rs;
                                }
                            }
                        });
                    });
            }
        });
    });
}

fn theme_toggle(ui: &mut Ui, state: &mut AppState, theme: &ThemeColors) {
    let theme_icon = if state.theme_mode == ThemeMode::Light {
        egui_phosphor::regular::MOON
    } else {
        egui_phosphor::regular::SUN
    };
    let toggle = ui
        .add(
            egui::Button::new(
                RichText::new(theme_icon)
                    .size(16.0)
                    .color(theme.on_surface_variant),
            )
            .fill(theme.surface_elevated)
            .stroke(egui::Stroke::new(1.0, theme.outline))
            .corner_radius(egui::CornerRadius::same(6))
            .min_size(egui::Vec2::splat(32.0)),
        )
        .on_hover_text(tr("Switch Light / Dark theme"))
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if toggle.clicked() {
        state.theme_mode = if state.theme_mode == ThemeMode::Light {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        };
    }
}

/// Breadcrumb header for the transcript document viewer.
fn draw_detail_topbar(ui: &mut Ui, state: &mut AppState, theme: &ThemeColors) {
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        let back = ui
            .add(
                egui::Button::new(
                    RichText::new(format!(
                        "{}  {}",
                        egui_phosphor::regular::ARROW_LEFT,
                        tr("Transcripts")
                    ))
                    .size(13.0)
                    .color(theme.on_surface_variant),
                )
                .fill(Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        if back.clicked() {
            state.current_screen = Screen::Transcripts;
        }
        ui.label(RichText::new("/").size(14.0).color(theme.outline));
        ui.label(
            RichText::new(&state.transcript_detail.title)
                .size(16.0)
                .family(crate::theme::bold_family())
                .color(theme.on_surface),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            theme_toggle(ui, state, theme);
            for (fmt, format) in [
                ("JSON", crate::export::ExportFormat::Json),
                ("SRT", crate::export::ExportFormat::Srt),
                ("TXT", crate::export::ExportFormat::Txt),
            ] {
                let label = format!("{} {fmt}", tr("Export"));
                let resp = ui
                    .add(
                        egui::Button::new(
                            RichText::new(label)
                                .font(egui::FontId::monospace(12.0))
                                .color(theme.on_surface),
                        )
                        .fill(theme.surface_elevated)
                        .stroke(egui::Stroke::new(1.0, theme.outline))
                        .corner_radius(egui::CornerRadius::same(6))
                        .min_size(egui::vec2(0.0, 32.0)),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if resp.clicked() {
                    match state.transcript_detail.dir.clone() {
                        Some(dir) if state.transcript_detail.live => {
                            state.export_transcript(&dir, format)
                        }
                        _ => crate::toast::show(ui.ctx(), crate::i18n::export_not_wired(fmt)),
                    }
                }
            }
            let copy = ui
                .add(
                    egui::Button::new(
                        RichText::new(format!(
                            "{}  {}",
                            egui_phosphor::regular::COPY,
                            tr("Copy all")
                        ))
                        .size(12.0)
                        .family(crate::theme::bold_family())
                        .color(theme.on_surface),
                    )
                    .fill(theme.surface_elevated)
                    .stroke(egui::Stroke::new(1.0, theme.outline))
                    .corner_radius(egui::CornerRadius::same(6))
                    .min_size(egui::vec2(0.0, 32.0)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if copy.clicked() {
                let text =
                    crate::screens::transcript_detail::transcript_as_text(&state.transcript_detail);
                ui.ctx().copy_text(text);
                crate::toast::show(ui.ctx(), tr("Transcript copied to clipboard"));
            }
        });
    });
}

/// Makes `rect` drag the window; double-click toggles maximise.
pub fn window_drag_region(ui: &mut Ui, rect: egui::Rect, id: &str) {
    let resp = ui.interact(rect, ui.id().with(id), egui::Sense::click_and_drag());
    if resp.drag_started_by(egui::PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if resp.double_clicked() {
        toggle_maximized(ui.ctx());
    }
}

fn toggle_maximized(ctx: &egui::Context) {
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
}

/// Width reserved at the right end of the top bar for [`draw_window_controls`].
pub const WINDOW_CONTROLS_WIDTH: f32 = 128.0;

/// Minimise / maximise-restore / close: rounded 36x32 buttons inset 8px from the window's
/// top-right corner, with a gap to the top-bar content (see [`WINDOW_CONTROLS_WIDTH`]).
pub fn draw_window_controls(ctx: &egui::Context, theme: &ThemeColors) {
    const W: f32 = 36.0;
    const H: f32 = 32.0;
    const GAP: f32 = 2.0;
    const INSET: f32 = 8.0;
    let screen = ctx.screen_rect();
    let total_w = 3.0 * W + 2.0 * GAP;
    egui::Area::new(egui::Id::new("window_controls"))
        .order(egui::Order::Tooltip)
        .fixed_pos(egui::pos2(
            screen.right() - INSET - total_w,
            screen.top() + INSET,
        ))
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(GAP, 0.0);
            let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
            ui.horizontal(|ui| {
                let buttons = [
                    (egui_phosphor::regular::MINUS, tr("Minimize"), false),
                    (
                        if maximized {
                            egui_phosphor::regular::COPY_SIMPLE
                        } else {
                            egui_phosphor::regular::SQUARE
                        },
                        if maximized {
                            tr("Restore")
                        } else {
                            tr("Maximize")
                        },
                        false,
                    ),
                    (egui_phosphor::regular::X, tr("Close"), true),
                ];
                for (i, (icon, tip, is_close)) in buttons.into_iter().enumerate() {
                    let (rect, resp) =
                        ui.allocate_exact_size(egui::vec2(W, H), egui::Sense::click());
                    let hovered = resp.hovered();
                    let (bg, fg) = match (hovered, is_close) {
                        (true, true) => (theme.error, Color32::WHITE),
                        (true, false) => (theme.surface_elevated, theme.on_surface),
                        (false, _) => (Color32::TRANSPARENT, theme.on_surface_variant),
                    };
                    ui.painter()
                        .rect_filled(rect, egui::CornerRadius::same(6), bg);
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        icon,
                        egui::FontId::proportional(15.0),
                        fg,
                    );
                    if resp.on_hover_text(tip).clicked() {
                        match i {
                            0 => ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)),
                            1 => toggle_maximized(ctx),
                            _ => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                        }
                    }
                }
            });
        });
}

/// Edge/corner resizing for the frameless window (the OS border is gone).
pub fn handle_window_resize(ctx: &egui::Context) {
    use egui::viewport::ResizeDirection as Dir;
    use egui::CursorIcon as Cur;

    const EDGE: f32 = 5.0;
    if ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
        return;
    }
    let Some(pos) = ctx.input(|i| i.pointer.latest_pos()) else {
        return;
    };
    let r = ctx.screen_rect();
    let left = pos.x - r.left() < EDGE;
    let right = r.right() - pos.x < EDGE;
    let top = pos.y - r.top() < EDGE;
    let bottom = r.bottom() - pos.y < EDGE;

    let (dir, cursor) = match (left, right, top, bottom) {
        (true, _, true, _) => (Dir::NorthWest, Cur::ResizeNwSe),
        (_, true, true, _) => (Dir::NorthEast, Cur::ResizeNeSw),
        (true, _, _, true) => (Dir::SouthWest, Cur::ResizeNeSw),
        (_, true, _, true) => (Dir::SouthEast, Cur::ResizeNwSe),
        (true, ..) => (Dir::West, Cur::ResizeHorizontal),
        (_, true, ..) => (Dir::East, Cur::ResizeHorizontal),
        (_, _, true, _) => (Dir::North, Cur::ResizeVertical),
        (_, _, _, true) => (Dir::South, Cur::ResizeVertical),
        _ => return,
    };
    ctx.set_cursor_icon(cursor);
    if ctx.input(|i| i.pointer.primary_pressed()) {
        ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
    }
}
