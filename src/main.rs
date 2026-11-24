mod app;
#[allow(dead_code)] // wired into the UI in a later task
mod asr;
mod audio;
mod branding;
mod components;
mod i18n;
mod modals;
mod screens;
mod settings_store;
mod shell;
mod theme;
mod toast;

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([1200.0, 760.0])
        // Custom title bar (see `shell::draw_window_controls`): the OS caption is a
        // different colour and shows a title string the design does not have.
        .with_decorations(false)
        .with_title("Meeting Recorder");
    if let Some(icon) = branding::window_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Meeting Recorder",
        options,
        Box::new(|cc| {
            theme::configure_fonts(&cc.egui_ctx);
            #[cfg(windows)]
            round_window_corners(cc);
            let args: Vec<String> = std::env::args().skip(1).collect();
            let mut state = app::AppState::from_args(args.clone().into_iter());
            // Dev/preview flags (`--screen=...`) are for reproducible screenshots: they neither
            // read nor overwrite the user's saved preferences.
            let dev_flags = args.iter().any(|a| a.starts_with("--"));
            if !dev_flags {
                state.enable_persistence(settings_store::load());
            }
            // `--audio` turns the capture worker on for a dev run (real device lists) without
            // touching the saved preferences.
            #[cfg(windows)]
            if !dev_flags || args.iter().any(|a| a == "--audio") {
                state.enable_audio(audio::worker::AudioHandle::spawn(
                    std::sync::Arc::new(audio::wasapi_backend::WasapiBackend::new()),
                    audio::worker::WorkerConfig::default(),
                ));
            }
            Ok(Box::new(state))
        }),
    )
}

/// Frameless windows get square corners on Windows 11 unless asked: request DWM's round style.
#[cfg(windows)]
fn round_window_corners(cc: &eframe::CreationContext<'_>) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    };

    let Ok(handle) = cc.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(win) = handle.as_raw() else {
        return;
    };
    let preference: i32 = DWMWCP_ROUND;
    // SAFETY: `hwnd` is the live window owned by eframe, and `preference` is a valid
    // DWM_WINDOW_CORNER_PREFERENCE value of the size we pass. Failure (Windows 10, or
    // older Windows 11 builds) is ignored: the window just keeps square corners.
    unsafe {
        DwmSetWindowAttribute(
            win.hwnd.get() as _,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            std::ptr::from_ref(&preference).cast(),
            std::mem::size_of::<i32>() as u32,
        );
    }
}
