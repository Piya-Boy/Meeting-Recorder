use egui::{RichText, Ui};

use super::{card, page, row, select};
use crate::components::button::{custom_button, ButtonVariant};
use crate::components::toggle::toggle_switch;
use crate::theme::ThemeColors;
use crate::toast;

const SYS_DEVICES: [&str; 2] = [
    "Speakers (Realtek(R) Audio) — WASAPI Loopback",
    "Headphones (Sony WH-1000XM5)",
];
const MIC_DEVICES: [&str; 2] = [
    "Shure MV7 USB Podcast Microphone",
    "Microphone Array (Intel Smart Sound)",
];
const SAMPLE_RATES: [&str; 3] = ["48,000 Hz", "44,100 Hz", "16,000 Hz"];
const CHANNELS: [&str; 3] = ["Separate tracks (2-ch)", "Stereo mixed", "Mono downmix"];

pub struct SettingsAudioScreen {
    pub sys_audio_enabled: bool,
    pub sys_device_idx: usize,
    pub mic_enabled: bool,
    pub mic_device_idx: usize,
    pub sample_rate_idx: usize,
    pub channel_idx: usize,
}

impl Default for SettingsAudioScreen {
    fn default() -> Self {
        Self {
            sys_audio_enabled: true,
            sys_device_idx: 0,
            mic_enabled: true,
            mic_device_idx: 0,
            sample_rate_idx: 0,
            channel_idx: 0,
        }
    }
}

pub fn draw_settings_audio(ui: &mut Ui, screen: &mut SettingsAudioScreen, theme: &ThemeColors) {
    page(
        ui,
        theme,
        "Audio",
        "Configure Windows WASAPI loopback and microphone capture devices, format, and hardware tests.",
        |ui| {
            device_card(
                ui,
                theme,
                egui_phosphor::regular::SPEAKER_HIGH,
                "Enable System Audio recording",
                "sys_device",
                &SYS_DEVICES,
                (&mut screen.sys_audio_enabled, &mut screen.sys_device_idx),
                "Test system audio",
            );
            device_card(
                ui,
                theme,
                egui_phosphor::regular::MICROPHONE,
                "Enable Microphone recording",
                "mic_device",
                &MIC_DEVICES,
                (&mut screen.mic_enabled, &mut screen.mic_device_idx),
                "Test microphone",
            );

            card(ui, theme, |ui| {
                row(
                    ui,
                    theme,
                    "Sample rate",
                    Some("48,000 Hz matches Windows WASAPI shared mode and resamples cleanly for OpenThai ASR."),
                    170.0,
                    |ui| select(ui, theme, "sample_rate", &mut screen.sample_rate_idx, &SAMPLE_RATES, 150.0, true),
                );
                row(
                    ui,
                    theme,
                    "Channel configuration",
                    Some("Separate audio tracks preserve System Audio and Microphone independently."),
                    200.0,
                    |ui| select(ui, theme, "channels", &mut screen.channel_idx, &CHANNELS, 180.0, false),
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
    devices: &[&str],
    (enabled, device_idx): (&mut bool, &mut usize),
    test_label: &str,
) {
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
                toggle_switch(ui, theme, enabled);
            });
        });
        ui.add_enabled_ui(*enabled, |ui| {
            // Right-to-left: the test button hugs the card edge, the dropdown takes the rest.
            let row_size = egui::vec2(ui.available_width(), 36.0);
            ui.allocate_ui_with_layout(
                row_size,
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if custom_button(ui, theme, None, test_label, ButtonVariant::Secondary)
                        .clicked()
                    {
                        toast::show(ui.ctx(), "Hardware test is not wired up yet (UI preview)");
                    }
                    let select_w = ui.available_width().max(120.0);
                    select(ui, theme, id, device_idx, devices, select_w, false);
                },
            );
        });
    });
}
