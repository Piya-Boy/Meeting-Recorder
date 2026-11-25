use egui::{RichText, Ui};

use super::{card, page, row, select};
use crate::audio::session::DeviceInfo;
use crate::components::select::dropdown;
use crate::components::toggle::toggle_switch;
use crate::screens::recorder::RecorderScreen;
use crate::theme::ThemeColors;

/// The first entry of every device dropdown; stored as an empty ID.
const WINDOWS_DEFAULT: &str = "Windows default";

/// The devices Windows reports and the ones the user picked. Shared by Settings > Audio and
/// the Recorder screen. Devices are stored by ID, so a list that changes order never moves a
/// selection; an ID that is not in the list shows as the Windows default but is kept, so the
/// choice returns when the device does.
#[derive(Default)]
pub struct AudioChoice {
    /// System render devices (captured as loopback) and microphones, as last reported.
    pub system: Vec<DeviceInfo>,
    pub mic: Vec<DeviceInfo>,
    pub system_id: String,
    pub mic_id: String,
    /// True once the worker has reported the device lists.
    pub devices_loaded: bool,
}

/// Sample rate in Hz for a position of the sample-rate dropdown.
pub fn sample_rate_hz(index: usize) -> u32 {
    match index {
        1 => 44_100,
        2 => 16_000,
        _ => 48_000,
    }
}

/// Dropdown position of `id`: 0 is the Windows default (also for an empty or unknown ID),
/// devices follow from 1.
pub fn selected_index(devices: &[DeviceInfo], id: &str) -> usize {
    if id.is_empty() {
        return 0;
    }
    devices.iter().position(|d| d.id == id).map_or(0, |i| i + 1)
}

fn labels(devices: &[DeviceInfo]) -> Vec<String> {
    std::iter::once(WINDOWS_DEFAULT.to_owned())
        .chain(devices.iter().map(|d| d.name.clone()))
        .collect()
}

fn id_at(devices: &[DeviceInfo], index: usize) -> String {
    match index {
        0 => String::new(),
        n => devices.get(n - 1).map(|d| d.id.clone()).unwrap_or_default(),
    }
}

fn some_id(id: &str) -> Option<String> {
    (!id.is_empty()).then(|| id.to_owned())
}

impl AudioChoice {
    pub fn apply_devices(&mut self, system: Vec<DeviceInfo>, mic: Vec<DeviceInfo>) {
        self.system = system;
        self.mic = mic;
        self.devices_loaded = true;
    }

    pub fn system_labels(&self) -> Vec<String> {
        labels(&self.system)
    }

    pub fn mic_labels(&self) -> Vec<String> {
        labels(&self.mic)
    }

    pub fn system_index(&self) -> usize {
        selected_index(&self.system, &self.system_id)
    }

    pub fn mic_index(&self) -> usize {
        selected_index(&self.mic, &self.mic_id)
    }

    pub fn select_system(&mut self, index: usize) {
        self.system_id = id_at(&self.system, index);
    }

    pub fn select_mic(&mut self, index: usize) {
        self.mic_id = id_at(&self.mic, index);
    }

    /// The chosen device ID, or `None` for the Windows default.
    pub fn system_device(&self) -> Option<String> {
        some_id(&self.system_id)
    }

    pub fn mic_device(&self) -> Option<String> {
        some_id(&self.mic_id)
    }
}

/// A device dropdown over `labels`; returns the new index when the user picked another one.
pub fn device_dropdown(
    ui: &mut Ui,
    theme: &ThemeColors,
    id: &str,
    labels: &[String],
    current: usize,
    width: f32,
) -> Option<usize> {
    let mut index = current;
    dropdown(
        ui,
        theme,
        id,
        &mut index,
        labels,
        width,
        theme.background,
        false,
    );
    (index != current).then_some(index)
}
const SAMPLE_RATES: [&str; 3] = ["48,000 Hz", "44,100 Hz", "16,000 Hz"];

#[derive(Default)]
pub struct SettingsAudioScreen {
    pub sample_rate_idx: usize,
}

pub fn draw_settings_audio(
    ui: &mut Ui,
    screen: &mut SettingsAudioScreen,
    choice: &mut AudioChoice,
    // The two source switches are the Recorder page's own, so both pages show one setting.
    recorder: &mut RecorderScreen,
    theme: &ThemeColors,
) {
    let locked = recorder.sources_locked();
    page(
        ui,
        theme,
        "Audio",
        "Choose the Windows WASAPI capture devices and the sample rate. The two sources are saved as separate files.",
        |ui| {
            let picked = device_card(
                ui,
                theme,
                egui_phosphor::regular::SPEAKER_HIGH,
                "Enable System Audio recording",
                "sys_device",
                (&choice.system_labels(), choice.system_index()),
                (&mut recorder.sys_audio_enabled, locked),
            );
            if let Some(i) = picked {
                choice.select_system(i);
            }
            let picked = device_card(
                ui,
                theme,
                egui_phosphor::regular::MICROPHONE,
                "Enable Microphone recording",
                "mic_device",
                (&choice.mic_labels(), choice.mic_index()),
                (&mut recorder.mic_enabled, locked),
            );
            if let Some(i) = picked {
                choice.select_mic(i);
            }

            card(ui, theme, |ui| {
                row(
                    ui,
                    theme,
                    "Sample rate",
                    Some("48,000 Hz matches Windows WASAPI shared mode and resamples cleanly for OpenThai ASR."),
                    170.0,
                    |ui| select(ui, theme, "sample_rate", &mut screen.sample_rate_idx, &SAMPLE_RATES, 150.0, true),
                );
            });
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn device_card(
    ui: &mut Ui,
    theme: &ThemeColors,
    icon: &str,
    title: &str,
    id: &str,
    (labels, current): (&[String], usize),
    (enabled, locked): (&mut bool, bool),
) -> Option<usize> {
    let mut picked = None;
    card(ui, theme, |ui| {
        ui.spacing_mut().item_spacing.y = 12.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.label(RichText::new(icon).size(16.0).color(theme.brand_blue));
            ui.label(
                RichText::new(crate::i18n::tr(title))
                    .size(14.0)
                    .family(crate::theme::bold_family())
                    .color(theme.on_surface),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if locked {
                    ui.disable();
                }
                toggle_switch(ui, theme, enabled);
            });
        });
        ui.add_enabled_ui(*enabled && !locked, |ui| {
            let width = ui.available_width();
            picked = device_dropdown(ui, theme, id, labels, current, width);
        });
    });
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(id: &str, name: &str) -> DeviceInfo {
        DeviceInfo {
            id: id.to_owned(),
            name: name.to_owned(),
            is_default: false,
        }
    }

    #[test]
    fn selected_index_default_for_empty_and_unknown() {
        let devices = [dev("a", "A"), dev("b", "B")];
        assert_eq!(selected_index(&devices, ""), 0);
        assert_eq!(selected_index(&devices, "gone"), 0);
        assert_eq!(selected_index(&[], "a"), 0);
    }

    #[test]
    fn selected_index_finds_device_after_default_entry() {
        let devices = [dev("a", "A"), dev("b", "B")];
        assert_eq!(selected_index(&devices, "a"), 1);
        assert_eq!(selected_index(&devices, "b"), 2);
    }

    #[test]
    fn labels_start_with_windows_default() {
        let mut c = AudioChoice::default();
        assert_eq!(c.system_labels(), vec!["Windows default".to_owned()]);
        c.apply_devices(
            vec![dev("s1", "Speakers")],
            vec![dev("m1", "Mic 1"), dev("m2", "Mic 2")],
        );
        assert_eq!(c.system_labels(), vec!["Windows default", "Speakers"]);
        assert_eq!(c.mic_labels(), vec!["Windows default", "Mic 1", "Mic 2"]);
    }

    #[test]
    fn selecting_by_index_stores_the_device_id() {
        let mut c = AudioChoice::default();
        c.apply_devices(
            vec![dev("s1", "Speakers")],
            vec![dev("m1", "Mic 1"), dev("m2", "Mic 2")],
        );
        c.select_mic(2);
        assert_eq!(c.mic_id, "m2");
        assert_eq!(c.mic_device(), Some("m2".to_owned()));
        assert_eq!(c.mic_index(), 2);
        c.select_mic(0);
        assert_eq!(c.mic_id, "");
        assert_eq!(c.mic_device(), None);
        c.select_system(1);
        assert_eq!(c.system_device(), Some("s1".to_owned()));
    }

    #[test]
    fn saved_id_missing_from_list_is_kept_but_shows_default() {
        let mut c = AudioChoice {
            mic_id: "unplugged".to_owned(),
            ..AudioChoice::default()
        };
        c.apply_devices(vec![], vec![dev("m1", "Mic 1")]);
        assert_eq!(c.mic_index(), 0);
        assert_eq!(c.mic_id, "unplugged");
        // The device comes back: it is selected again.
        c.apply_devices(
            vec![],
            vec![dev("m1", "Mic 1"), dev("unplugged", "Headset")],
        );
        assert_eq!(c.mic_index(), 2);
    }

    #[test]
    fn sample_rate_follows_the_dropdown_order() {
        assert_eq!(sample_rate_hz(0), 48_000);
        assert_eq!(sample_rate_hz(1), 44_100);
        assert_eq!(sample_rate_hz(2), 16_000);
        assert_eq!(
            sample_rate_hz(99),
            48_000,
            "unknown index falls back to the default"
        );
    }
}
