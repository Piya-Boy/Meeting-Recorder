use std::path::PathBuf;

use crate::i18n::tr;
use crate::library;
use egui::{Color32, RichText, Ui};

use crate::app::{Modal, Screen};
use crate::components::button::{
    compact_button, custom_button, icon_button, mono_chip_button, sized_icon_button, ButtonVariant,
};
use crate::components::input::search_box;
use crate::components::select::dropdown;
use crate::components::table::{body_row, header_row, table_frame, Align, Column};
use crate::theme::ThemeColors;

#[derive(Clone, Debug)]
pub struct RecordingEntry {
    pub id: usize,
    pub display_name: String,
    pub file_name: String,
    pub date: String,
    pub duration: String,
    pub size: String,
    pub status: RecordingStatus,
    /// The session folder; `None` for the preview's sample rows.
    pub dir: Option<PathBuf>,
    /// `YYYY-MM-DD HH:MM` for the date filter and the sort.
    pub stamp: String,
    pub duration_s: u64,
    pub bytes: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum RecordingStatus {
    Transcribed,
    Processing(u8), // percent
    #[default]
    AudioOnly,
}

/// What a row button asks the app to do (taken once per frame).
#[derive(Clone, Debug, PartialEq)]
pub enum RecordingsRequest {
    OpenTranscript(PathBuf),
    Transcribe(PathBuf),
    OpenFolder(PathBuf),
    /// Play, pause or resume this recording (starts it if another one is playing).
    TogglePlay(PathBuf),
    /// Move the player to this fraction (0..=1) of the recording.
    Seek(f32),
    StopPlayback,
}

/// What the bottom player bar shows for the recording that is loaded in the player.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerView {
    pub dir: PathBuf,
    pub name: String,
    pub playing: bool,
    pub position_s: f32,
    pub total_s: f32,
}

impl PlayerView {
    /// How far into the recording the player is, 0..=1 (0 while the length is unknown).
    pub fn fraction(&self) -> f32 {
        if self.total_s <= 0.0 {
            0.0
        } else {
            (self.position_s / self.total_s).clamp(0.0, 1.0)
        }
    }
}

impl RecordingStatus {
    pub fn label(&self) -> String {
        match self {
            Self::Transcribed => tr("Transcribed").to_owned(),
            Self::Processing(pct) => format!("{} {}%", tr("Processing"), pct),
            Self::AudioOnly => tr("Audio only").to_owned(),
        }
    }
}

pub struct RecordingsScreen {
    pub search: String,
    pub date_filter_idx: usize,
    pub sort_idx: usize,
    pub recordings: Vec<RecordingEntry>,
    pub playback_id: Option<usize>,
    pub playback_position: f32,
    pub date_filters: Vec<&'static str>,
    pub sort_options: Vec<&'static str>,
    pub show_empty: bool,
    /// True once the app lists real folders; false keeps the sample rows (preview mode).
    pub live: bool,
    /// False until the first scan arrives in live mode.
    pub loaded: bool,
    /// The folder being transcribed and its percent, shown as Processing.
    pub processing: Option<(PathBuf, u8)>,
    /// Set by a row button; taken by the app.
    pub request: Option<RecordingsRequest>,
    /// The loaded player, copied from the app every frame (live mode only).
    pub player: Option<PlayerView>,
    /// Fraction under the pointer while the seek bar is being dragged.
    seeking: Option<f32>,
    entries: Vec<library::Entry>,
}

impl RecordingsScreen {
    /// Switches to real data; the sample rows go.
    pub fn go_live(&mut self) {
        self.live = true;
        self.loaded = false;
        self.recordings.clear();
        self.playback_id = None;
    }

    /// Shows a new scan.
    pub fn set_entries(&mut self, entries: Vec<library::Entry>) {
        self.entries = entries;
        self.loaded = true;
        self.rebuild_rows();
    }

    /// The folder whose transcription is running or waiting (with its percent), if any.
    pub fn set_processing(&mut self, processing: Option<(PathBuf, u8)>) {
        if self.processing != processing {
            self.processing = processing;
            if self.live {
                self.rebuild_rows();
            }
        }
    }

    fn rebuild_rows(&mut self) {
        let processing = self.processing.clone();
        self.recordings = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let status = if e.transcript.is_some() {
                    RecordingStatus::Transcribed
                } else if let Some((_, pct)) = processing.as_ref().filter(|(d, _)| *d == e.dir) {
                    RecordingStatus::Processing(*pct)
                } else {
                    RecordingStatus::AudioOnly
                };
                RecordingEntry {
                    id: i + 1,
                    display_name: e.name.clone(),
                    file_name: e.folder.clone(),
                    date: e.stamp.clone(),
                    duration: library::format_duration(e.duration_s),
                    size: library::format_size(e.bytes),
                    status,
                    dir: Some(e.dir.clone()),
                    stamp: e.stamp.clone(),
                    duration_s: e.duration_s,
                    bytes: e.bytes,
                }
            })
            .collect();
    }

    /// Indexes into `recordings` that pass the search and date filter, in the chosen order.
    /// `today` is `YYYY-MM-DD`.
    pub fn visible(&self, today: &str) -> Vec<usize> {
        let query = self.search.trim().to_lowercase();
        let today_n = library::day_number(today);
        let max_age = match self.date_filter_idx {
            1 => Some(0),
            2 => Some(6),
            3 => Some(29),
            _ => None,
        };
        let mut out: Vec<usize> = self
            .recordings
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                query.is_empty()
                    || r.display_name.to_lowercase().contains(&query)
                    || r.file_name.to_lowercase().contains(&query)
            })
            .filter(|(_, r)| match (max_age, today_n) {
                (Some(max), Some(t)) => {
                    library::day_number(&r.stamp).is_some_and(|d| d <= t && t - d <= max)
                }
                _ => true,
            })
            .map(|(i, _)| i)
            .collect();
        let rows = &self.recordings;
        match self.sort_idx {
            1 => out.sort_by(|&a, &b| rows[a].stamp.cmp(&rows[b].stamp)),
            2 => out.sort_by(|&a, &b| rows[b].duration_s.cmp(&rows[a].duration_s)),
            3 => out.sort_by(|&a, &b| rows[b].bytes.cmp(&rows[a].bytes)),
            _ => out.sort_by(|&a, &b| rows[b].stamp.cmp(&rows[a].stamp)),
        }
        out
    }
}

impl Default for RecordingsScreen {
    fn default() -> Self {
        Self {
            search: String::new(),
            date_filter_idx: 0,
            sort_idx: 0,
            show_empty: false,
            live: false,
            loaded: true,
            processing: None,
            request: None,
            player: None,
            seeking: None,
            entries: Vec::new(),
            recordings: vec![
                RecordingEntry {
                    id: 1,
                    display_name: "Q4 Engineering Architecture Sync".to_owned(),
                    file_name: "2026-11-23_q4-architecture-sync.wav".to_owned(),
                    date: "2026-11-23 09:30".to_owned(),
                    duration: "00:42:18".to_owned(),
                    size: "184.6 MB".to_owned(),
                    status: RecordingStatus::Transcribed,
                    dir: None,
                    stamp: "2026-11-23 09:30".to_owned(),
                    duration_s: 2538,
                    bytes: 193580000,
                },
                RecordingEntry {
                    id: 2,
                    display_name: "Bangkok Client Onboarding Review".to_owned(),
                    file_name: "2026-11-22_client-onboarding.wav".to_owned(),
                    date: "2026-11-22 14:00".to_owned(),
                    duration: "01:08:44".to_owned(),
                    size: "301.2 MB".to_owned(),
                    status: RecordingStatus::Transcribed,
                    dir: None,
                    stamp: "2026-11-22 14:00".to_owned(),
                    duration_s: 4124,
                    bytes: 315800000,
                },
                RecordingEntry {
                    id: 3,
                    display_name: "OpenThai ASR Latency Benchmark".to_owned(),
                    file_name: "2026-11-21_asr-latency.wav".to_owned(),
                    date: "2026-11-21 16:15".to_owned(),
                    duration: "00:19:05".to_owned(),
                    size: "83.4 MB".to_owned(),
                    status: RecordingStatus::Processing(78),
                    dir: None,
                    stamp: "2026-11-21 16:15".to_owned(),
                    duration_s: 1145,
                    bytes: 87400000,
                },
                RecordingEntry {
                    id: 4,
                    display_name: "Weekly Product Design Critique".to_owned(),
                    file_name: "2026-11-19_design-critique.wav".to_owned(),
                    date: "2026-11-19 11:00".to_owned(),
                    duration: "00:34:52".to_owned(),
                    size: "152.1 MB".to_owned(),
                    status: RecordingStatus::AudioOnly,
                    dir: None,
                    stamp: "2026-11-19 11:00".to_owned(),
                    duration_s: 2092,
                    bytes: 159500000,
                },
            ],
            playback_id: Some(1),
            playback_position: 0.30,
            date_filters: vec![
                tr("All dates"),
                tr("Today"),
                tr("Last 7 days"),
                tr("Last 30 days"),
            ],
            sort_options: vec![
                tr("Sort: Date (Newest)"),
                tr("Sort: Date (Oldest)"),
                tr("Sort: Duration"),
                tr("Sort: File size"),
            ],
        }
    }
}

const COLS: [Column; 6] = [
    Column::flex(),
    Column::fixed(130.0, Align::Left),
    Column::fixed(80.0, Align::Right),
    Column::fixed(80.0, Align::Right),
    Column::fixed(128.0, Align::Left),
    Column::fixed(236.0, Align::Right),
];

/// Draws the recordings library. Returns a screen to navigate to, if any.
pub fn draw_recordings_screen(
    ui: &mut Ui,
    theme: &ThemeColors,
    screen: &mut RecordingsScreen,
    open_modal: &mut Option<Modal>,
) -> Option<Screen> {
    let mut navigate = None;

    // Playback bar pinned to the bottom (hidden in the empty state).
    let showing_list = !(screen.show_empty || screen.recordings.is_empty());
    let bar_visible = if screen.live {
        screen.player.is_some()
    } else {
        screen.playback_id.is_some()
    };
    if showing_list && bar_visible {
        egui::TopBottomPanel::bottom("recordings_playback")
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                left: 24,
                right: 24,
                top: 0,
                bottom: 24,
            }))
            .show_inside(ui, |ui| draw_playback_bar(ui, theme, screen));
    }

    egui::CentralPanel::default()
        .frame(
            egui::Frame::NONE
                .fill(theme.background)
                .inner_margin(egui::Margin::same(24)),
        )
        .show_inside(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 16.0;
            draw_toolbar(ui, theme, screen);

            if screen.live && !screen.loaded {
                ui.label(
                    RichText::new(tr("Loading recordings..."))
                        .size(13.0)
                        .color(theme.on_surface_variant),
                );
            } else if showing_list {
                if let Some(target) = draw_recordings_table(ui, theme, screen, open_modal) {
                    navigate = Some(target);
                }
            } else if draw_empty_state(ui, theme) {
                navigate = Some(Screen::Recorder);
            }
        });

    navigate
}

fn draw_toolbar(ui: &mut Ui, theme: &ThemeColors, screen: &mut RecordingsScreen) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        search_box(
            ui,
            theme,
            &mut screen.search,
            tr("Search recordings by name..."),
            320.0,
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            dropdown(
                ui,
                theme,
                "sort_combo",
                &mut screen.sort_idx,
                &screen.sort_options,
                176.0,
                theme.surface,
                false,
            );
            dropdown(
                ui,
                theme,
                "date_combo",
                &mut screen.date_filter_idx,
                &screen.date_filters,
                112.0,
                theme.surface,
                false,
            );
        });
    });
}

fn draw_empty_state(ui: &mut Ui, theme: &ThemeColors) -> bool {
    let mut go = false;
    egui::Frame::NONE
        .fill(theme.surface)
        .stroke(egui::Stroke::new(1.0, theme.outline))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(64))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical_centered(|ui| {
                ui.spacing_mut().item_spacing.y = 12.0;
                let (icon_rect, _) =
                    ui.allocate_exact_size(egui::Vec2::splat(48.0), egui::Sense::hover());
                ui.painter().rect(
                    icon_rect,
                    egui::CornerRadius::same(8),
                    theme.surface_elevated,
                    egui::Stroke::new(1.0, theme.outline),
                    egui::StrokeKind::Inside,
                );
                ui.painter().text(
                    icon_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    egui_phosphor::regular::DISC,
                    egui::FontId::proportional(24.0),
                    theme.on_surface_variant,
                );
                ui.label(
                    RichText::new(tr("No recordings yet"))
                        .size(16.0)
                        .family(crate::theme::bold_family())
                        .color(theme.on_surface),
                );
                ui.label(
                    RichText::new(
                        "Start a new recording session to capture System Audio and\nMicrophone tracks locally on this PC.",
                    )
                    .size(13.0)
                    .color(theme.on_surface_variant),
                );
                ui.add_space(4.0);
                if custom_button(
                    ui,
                    theme,
                    Some(egui_phosphor::regular::MICROPHONE),
                    tr("Go to Recorder"),
                    ButtonVariant::Primary,
                )
                .clicked()
                {
                    go = true;
                }
            });
        });
    go
}

fn draw_recordings_table(
    ui: &mut Ui,
    theme: &ThemeColors,
    screen: &mut RecordingsScreen,
    open_modal: &mut Option<Modal>,
) -> Option<Screen> {
    let mut navigate = None;
    let mut to_delete: Option<(usize, String, Option<PathBuf>)> = None;
    let mut toggle_play: Option<usize> = None;
    let mut transcribe: Option<usize> = None;
    let mut request: Option<RecordingsRequest> = None;
    let live = screen.live;

    let today = crate::audio::naming::local_stamp();
    let visible: Vec<usize> = screen.visible(today.get(..10).unwrap_or(""));

    table_frame(ui, theme, |ui| {
        header_row(
            ui,
            theme,
            &COLS,
            &[
                tr("Recording name"),
                tr("Date"),
                tr("Duration"),
                tr("Size"),
                tr("Status"),
                tr("Actions"),
            ],
        );

        if visible.is_empty() {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new(tr("No recordings match your search."))
                        .size(13.0)
                        .color(theme.on_surface_variant),
                );
            });
            ui.add_space(24.0);
            return;
        }

        egui::ScrollArea::vertical()
            .id_salt("recordings_table")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let last_idx = visible.len() - 1;
                for (n, &idx) in visible.iter().enumerate() {
                    let entry = &screen.recordings[idx];
                    let is_playing = !live && screen.playback_id == Some(entry.id);
                    body_row(ui, theme, &COLS, 64.0, is_playing, n == last_idx, |cells| {
                        cells.stack(40.0, |ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&entry.display_name)
                                        .size(14.0)
                                        .family(crate::theme::bold_family())
                                        .color(theme.on_surface),
                                )
                                .truncate(),
                            );
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&entry.file_name)
                                        .font(egui::FontId::monospace(12.0))
                                        .color(theme.on_surface_variant),
                                )
                                .truncate(),
                            );
                        });
                        cells.cell(|ui| mono(ui, theme, &entry.date, false));
                        cells.cell(|ui| mono(ui, theme, &entry.duration, true));
                        cells.cell(|ui| mono(ui, theme, &entry.size, false));
                        cells.cell(|ui| status_pill(ui, theme, &entry.status));
                        cells.cell(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            // right_to_left: rightmost first
                            if icon_button(ui, theme, egui_phosphor::regular::TRASH, 26.0)
                                .on_hover_text(tr("Delete recording"))
                                .clicked()
                            {
                                to_delete =
                                    Some((entry.id, entry.display_name.clone(), entry.dir.clone()));
                            }
                            match &entry.status {
                                RecordingStatus::Transcribed => {
                                    if compact_button(
                                        ui,
                                        theme,
                                        Some(egui_phosphor::regular::FILE_TEXT),
                                        tr("Open transcript"),
                                        ButtonVariant::Secondary,
                                    )
                                    .clicked()
                                    {
                                        match &entry.dir {
                                            Some(dir) if live => {
                                                request = Some(RecordingsRequest::OpenTranscript(
                                                    dir.clone(),
                                                ))
                                            }
                                            _ => navigate = Some(Screen::TranscriptDetail),
                                        }
                                    }
                                }
                                RecordingStatus::Processing(_) => {
                                    ui.add_enabled(
                                        false,
                                        egui::Button::new(
                                            RichText::new(format!(
                                                "{}  {}",
                                                egui_phosphor::regular::FILE_TEXT,
                                                tr("Pending")
                                            ))
                                            .size(11.5),
                                        )
                                        .fill(theme.background)
                                        .stroke(egui::Stroke::new(1.0, theme.outline))
                                        .min_size(egui::vec2(0.0, 26.0)),
                                    );
                                }
                                RecordingStatus::AudioOnly => {
                                    if compact_button(
                                        ui,
                                        theme,
                                        Some(egui_phosphor::regular::SPARKLE),
                                        tr("Transcribe"),
                                        ButtonVariant::Secondary,
                                    )
                                    .clicked()
                                    {
                                        match &entry.dir {
                                            Some(dir) if live => {
                                                request =
                                                    Some(RecordingsRequest::Transcribe(dir.clone()))
                                            }
                                            _ => transcribe = Some(entry.id),
                                        }
                                    }
                                }
                            }
                            if let (true, Some(dir)) = (live, &entry.dir) {
                                if icon_button(ui, theme, egui_phosphor::regular::FOLDER_OPEN, 26.0)
                                    .on_hover_text(tr("Open folder"))
                                    .clicked()
                                {
                                    request = Some(RecordingsRequest::OpenFolder(dir.clone()));
                                }
                                let playing_this = screen
                                    .player
                                    .as_ref()
                                    .is_some_and(|p| p.playing && &p.dir == dir);
                                let (icon, variant, hint) = if playing_this {
                                    (
                                        egui_phosphor::regular::PAUSE,
                                        ButtonVariant::Primary,
                                        tr("Pause"),
                                    )
                                } else {
                                    (
                                        egui_phosphor::regular::PLAY,
                                        ButtonVariant::Secondary,
                                        tr("Play"),
                                    )
                                };
                                if sized_icon_button(ui, theme, icon, 26.0, variant)
                                    .on_hover_text(hint)
                                    .clicked()
                                {
                                    request = Some(RecordingsRequest::TogglePlay(dir.clone()));
                                }
                                return;
                            }
                            let (icon, variant) = if is_playing {
                                (egui_phosphor::regular::PAUSE, ButtonVariant::Primary)
                            } else {
                                (egui_phosphor::regular::PLAY, ButtonVariant::Secondary)
                            };
                            let play = sized_icon_button(ui, theme, icon, 26.0, variant);
                            if play.clicked() {
                                toggle_play = Some(entry.id);
                            }
                        });
                    });
                }
            });
    });

    if let Some((id, name, dir)) = to_delete {
        *open_modal = Some(Modal::DeleteRecording { id, name, dir });
    }
    if request.is_some() {
        screen.request = request;
    }
    if let Some(id) = toggle_play {
        screen.playback_id = if screen.playback_id == Some(id) {
            None
        } else {
            Some(id)
        };
    }
    if let Some(id) = transcribe {
        if let Some(r) = screen.recordings.iter_mut().find(|r| r.id == id) {
            r.status = RecordingStatus::Processing(0);
        }
    }
    navigate
}

fn mono(ui: &mut Ui, theme: &ThemeColors, text: &str, strong: bool) {
    let mut t = RichText::new(text)
        .font(egui::FontId::monospace(12.0))
        .color(if strong {
            theme.on_surface
        } else {
            theme.on_surface_variant
        });
    if strong {
        t = t.family(crate::theme::mono_bold_family());
    }
    ui.label(t);
}

fn status_pill(ui: &mut Ui, theme: &ThemeColors, status: &RecordingStatus) {
    let (bg, stroke, fg, icon) = match status {
        RecordingStatus::Transcribed => (
            Color32::from_rgba_unmultiplied(0x22, 0xC5, 0x5E, 32),
            egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(0x22, 0xC5, 0x5E, 70)),
            theme.success_text,
            egui_phosphor::regular::CHECK_CIRCLE,
        ),
        RecordingStatus::Processing(_) => (
            theme.primary_container,
            egui::Stroke::NONE,
            theme.on_primary_container,
            egui_phosphor::regular::SPINNER_GAP,
        ),
        RecordingStatus::AudioOnly => (
            theme.surface_elevated,
            egui::Stroke::new(1.0, theme.outline),
            theme.on_surface_variant,
            egui_phosphor::regular::MINUS,
        ),
    };
    egui::Frame::NONE
        .fill(bg)
        .stroke(stroke)
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{}  {}", icon, status.label()))
                        .size(11.5)
                        .family(crate::theme::bold_family())
                        .color(fg),
                );
            });
        });
}

/// Bottom player: `[play | title/track]  [elapsed ─── seek ─── total]  [speed | volume]`,
/// laid out like Stitch's `justify-between` flex row (middle group `max-w-xl`).
fn draw_playback_bar(ui: &mut Ui, theme: &ThemeColors, screen: &mut RecordingsScreen) {
    const BTN: f32 = 36.0;
    const GAP_MD: f32 = 12.0;
    const MID_MAX: f32 = 576.0;

    let live_view = if screen.live {
        screen.player.clone()
    } else {
        None
    };
    let mut request: Option<RecordingsRequest> = None;
    let title = match &live_view {
        Some(v) => v.name.clone(),
        None => screen
            .playback_id
            .and_then(|id| screen.recordings.iter().find(|e| e.id == id))
            .map(|e| e.display_name.clone())
            .unwrap_or_default(),
    };

    let title_font = egui::FontId::new(14.0, crate::theme::bold_family());
    let mono = egui::FontId::monospace(12.0);
    let (title_w, track_w) = ui.fonts(|f| {
        (
            f.layout_no_wrap(title.clone(), title_font.clone(), Color32::WHITE)
                .size()
                .x,
            f.layout_no_wrap(
                tr("Track: System + Mic Mix").to_owned(),
                mono.clone(),
                Color32::WHITE,
            )
            .size()
            .x,
        )
    });

    egui::Frame::NONE
        .fill(theme.surface)
        .stroke(egui::Stroke::new(1.0, theme.outline))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            let total = ui.available_width();
            ui.set_width(total);

            let right_w = 40.0 + 8.0 + 28.0; // speed chip + gap + volume
            let left_w = BTN + GAP_MD + title_w.max(track_w).min(300.0);
            let free = (total - left_w - right_w).max(0.0);
            let mid_w = (free - 2.0 * 16.0).clamp(0.0, MID_MAX);
            let gap = ((free - mid_w) / 2.0).max(0.0);

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;

                // Left: play/pause + title
                ui.spacing_mut().item_spacing.x = GAP_MD;
                let playing_now = live_view.as_ref().is_none_or(|v| v.playing);
                let icon = if playing_now {
                    egui_phosphor::regular::PAUSE
                } else {
                    egui_phosphor::regular::PLAY
                };
                if sized_icon_button(ui, theme, icon, BTN, ButtonVariant::Primary).clicked() {
                    match &live_view {
                        Some(v) => request = Some(RecordingsRequest::TogglePlay(v.dir.clone())),
                        None => screen.playback_id = None,
                    }
                }
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.set_width(left_w - BTN - GAP_MD);
                    ui.add(
                        egui::Label::new(
                            RichText::new(title.clone())
                                .size(14.0)
                                .family(crate::theme::bold_family())
                                .color(theme.on_surface),
                        )
                        .truncate(),
                    );
                    ui.label(
                        RichText::new(tr("Track: System + Mic Mix"))
                            .font(mono.clone())
                            .color(theme.on_surface_variant),
                    );
                });

                // Middle: elapsed, seek bar, total
                ui.add_space(gap - GAP_MD);
                ui.allocate_ui_with_layout(
                    egui::vec2(mid_w, BTN),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.spacing_mut().item_spacing.x = GAP_MD;
                        let elapsed = match &live_view {
                            Some(v) => library::format_duration(v.position_s as u64),
                            None => "00:12:40".to_owned(),
                        };
                        ui.label(
                            RichText::new(elapsed)
                                .font(mono.clone())
                                .color(theme.on_surface_variant),
                        );
                        let seek_w = (ui.available_width() - 56.0 - GAP_MD).max(40.0);
                        let (rect, resp) = ui.allocate_exact_size(
                            egui::vec2(seek_w, 14.0),
                            egui::Sense::click_and_drag(),
                        );
                        let at_pointer = |pos: egui::Pos2| {
                            ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0)
                        };
                        if live_view.is_some() {
                            // Drag shows the new place at once; the player moves on release.
                            if resp.dragged() {
                                screen.seeking = resp.interact_pointer_pos().map(at_pointer);
                            }
                            if resp.drag_stopped() {
                                if let Some(f) = screen.seeking.take() {
                                    request = Some(RecordingsRequest::Seek(f));
                                }
                            } else if resp.clicked() {
                                if let Some(pos) = resp.interact_pointer_pos() {
                                    request = Some(RecordingsRequest::Seek(at_pointer(pos)));
                                }
                            }
                        } else if let Some(pos) = resp.interact_pointer_pos() {
                            screen.playback_position = at_pointer(pos);
                        }
                        let shown_fraction = match &live_view {
                            Some(v) => screen.seeking.unwrap_or_else(|| v.fraction()),
                            None => screen.playback_position,
                        };
                        let track = egui::Rect::from_center_size(
                            rect.center(),
                            egui::vec2(rect.width(), 8.0),
                        );
                        ui.painter().rect(
                            track,
                            egui::CornerRadius::same(4),
                            theme.background,
                            egui::Stroke::new(1.0, theme.outline_variant),
                            egui::StrokeKind::Inside,
                        );
                        let fill = egui::Rect::from_min_size(
                            track.min,
                            egui::vec2(track.width() * shown_fraction, track.height()),
                        );
                        ui.painter().rect_filled(
                            fill,
                            egui::CornerRadius::same(4),
                            theme.brand_blue,
                        );
                        let total = match &live_view {
                            Some(v) => library::format_duration(v.total_s as u64),
                            None => "00:42:18".to_owned(),
                        };
                        ui.label(
                            RichText::new(total)
                                .font(mono.clone())
                                .color(theme.on_surface_variant),
                        );
                    },
                );

                // Right: speed chip + volume, pinned to the right edge
                ui.add_space((gap - GAP_MD).max(0.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if live_view.is_some() {
                        // Speed and volume are not wired: a close button instead.
                        if icon_button(ui, theme, egui_phosphor::regular::X, 28.0)
                            .on_hover_text(tr("Close player"))
                            .clicked()
                        {
                            request = Some(RecordingsRequest::StopPlayback);
                        }
                        return;
                    }
                    ui.label(
                        RichText::new(egui_phosphor::regular::SPEAKER_HIGH)
                            .size(16.0)
                            .color(theme.on_surface_variant),
                    );
                    mono_chip_button(ui, theme, "1.0x");
                });
            });
        });
    if request.is_some() {
        screen.request = request;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(
        folder: &str,
        stamp: &str,
        duration_s: u64,
        bytes: u64,
        transcribed: bool,
    ) -> library::Entry {
        library::Entry {
            dir: PathBuf::from(format!("C:\\rec\\{folder}")),
            folder: folder.to_owned(),
            name: folder.to_owned(),
            stamp: stamp.to_owned(),
            duration_s,
            bytes,
            transcript: transcribed.then(|| library::TranscriptInfo {
                lines: 1,
                preview: "x".into(),
                text: "x".into(),
            }),
        }
    }

    fn live_screen() -> RecordingsScreen {
        let mut s = RecordingsScreen::default();
        s.go_live();
        s.set_entries(vec![
            entry("standup", "2026-11-24 09:00", 600, 5_000, true),
            entry("retro", "2026-11-20 10:00", 3_000, 9_000, false),
            entry("old-review", "2026-10-01 10:00", 100, 20_000, false),
            entry("loose", "", 50, 1_000, false),
        ]);
        s
    }

    fn names(s: &RecordingsScreen, idx: Vec<usize>) -> Vec<String> {
        idx.into_iter()
            .map(|i| s.recordings[i].display_name.clone())
            .collect()
    }

    #[test]
    fn the_bar_fraction_comes_from_position_and_total() {
        let view = |position_s: f32, total_s: f32| PlayerView {
            dir: PathBuf::from("C:\\rec\\a"),
            name: "a".into(),
            playing: true,
            position_s,
            total_s,
        };
        assert_eq!(view(30.0, 120.0).fraction(), 0.25);
        assert_eq!(view(5.0, 0.0).fraction(), 0.0, "no length yet");
        assert_eq!(view(500.0, 120.0).fraction(), 1.0, "never past the end");
    }

    #[test]
    fn going_live_drops_the_sample_rows_until_a_scan_arrives() {
        let mut s = RecordingsScreen::default();
        assert!(!s.recordings.is_empty() && s.loaded);
        s.go_live();
        assert!(s.recordings.is_empty() && !s.loaded);
        s.set_entries(vec![]);
        assert!(s.loaded && s.recordings.is_empty());
    }

    #[test]
    fn rows_carry_real_values_and_the_folder() {
        let s = live_screen();
        let r = &s.recordings[0];
        assert_eq!(r.display_name, "standup");
        assert_eq!(r.date, "2026-11-24 09:00");
        assert_eq!(r.duration, "00:10:00");
        assert_eq!(r.size, "4 KB");
        assert_eq!(
            r.dir.as_deref(),
            Some(std::path::Path::new("C:\\rec\\standup"))
        );
        assert_eq!(r.status, RecordingStatus::Transcribed);
        assert_eq!(s.recordings[1].status, RecordingStatus::AudioOnly);
    }

    #[test]
    fn the_folder_being_transcribed_shows_processing() {
        let mut s = live_screen();
        s.set_processing(Some((PathBuf::from("C:\\rec\\retro"), 40)));
        assert_eq!(s.recordings[1].status, RecordingStatus::Processing(40));
        assert_eq!(s.recordings[2].status, RecordingStatus::AudioOnly);
        s.set_processing(Some((PathBuf::from("C:\\rec\\standup"), 10)));
        assert_eq!(
            s.recordings[0].status,
            RecordingStatus::Transcribed,
            "a transcript wins over a job"
        );
        s.set_processing(None);
        assert_eq!(s.recordings[1].status, RecordingStatus::AudioOnly);
    }

    #[test]
    fn visible_filters_by_search_date_and_sort() {
        let mut s = live_screen();
        let today = "2026-11-24";
        assert_eq!(
            names(&s, s.visible(today)),
            ["standup", "retro", "old-review", "loose"].map(String::from)
        );
        s.search = "RET".into();
        assert_eq!(names(&s, s.visible(today)), ["retro".to_owned()]);
        s.search.clear();
        s.date_filter_idx = 1; // today
        assert_eq!(names(&s, s.visible(today)), ["standup".to_owned()]);
        s.date_filter_idx = 2; // last 7 days
        assert_eq!(
            names(&s, s.visible(today)),
            ["standup", "retro"].map(String::from)
        );
        s.date_filter_idx = 3; // last 30 days
        assert_eq!(
            names(&s, s.visible(today)),
            ["standup", "retro"].map(String::from)
        );
        s.date_filter_idx = 0;
        s.sort_idx = 1; // oldest first
        assert_eq!(
            names(&s, s.visible(today)),
            ["loose", "old-review", "retro", "standup"].map(String::from)
        );
        s.sort_idx = 2; // longest
        assert_eq!(names(&s, s.visible(today))[0], "retro");
        s.sort_idx = 3; // biggest
        assert_eq!(names(&s, s.visible(today))[0], "old-review");
    }
}
