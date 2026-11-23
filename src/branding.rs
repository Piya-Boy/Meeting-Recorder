//! App logo: window/taskbar icon and the in-UI logo texture, both from `assets/icon/Icon.png`.

use egui::{Color32, Context, Rect, TextureHandle, Ui};
use image::imageops::{resize, FilterType};

const ICON_PNG: &[u8] = include_bytes!("../assets/icon/Icon.png");

/// Edge length of the in-UI logo texture: 4x the largest logo (36 px) so zoom/HiDPI stay sharp.
const LOGO_TEXTURE_PX: u32 = 144;
/// Edge length of the window/taskbar icon.
const WINDOW_ICON_PX: u32 = 256;

/// The source PNG is RGB with a pure-black backdrop around the rounded-square icon. Flood-fill
/// that backdrop from the borders into transparency so the icon sits cleanly on any surface.
fn decode() -> Option<image::RgbaImage> {
    const BACKDROP_MAX: u8 = 24;
    let mut img = image::load_from_memory_with_format(ICON_PNG, image::ImageFormat::Png)
        .ok()?
        .into_rgba8();
    let (w, h) = img.dimensions();
    let is_backdrop = |img: &image::RgbaImage, x: u32, y: u32| {
        let p = img.get_pixel(x, y).0;
        p[3] != 0 && p[0].max(p[1]).max(p[2]) <= BACKDROP_MAX
    };
    let mut stack: Vec<(u32, u32)> = Vec::new();
    for x in 0..w {
        stack.push((x, 0));
        stack.push((x, h - 1));
    }
    for y in 0..h {
        stack.push((0, y));
        stack.push((w - 1, y));
    }
    while let Some((x, y)) = stack.pop() {
        if !is_backdrop(&img, x, y) {
            continue;
        }
        img.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
        if x > 0 {
            stack.push((x - 1, y));
        }
        if y > 0 {
            stack.push((x, y - 1));
        }
        if x + 1 < w {
            stack.push((x + 1, y));
        }
        if y + 1 < h {
            stack.push((x, y + 1));
        }
    }
    Some(img)
}

/// Resizes with premultiplied alpha so transparent pixels do not bleed dark fringes into the
/// icon edge, then returns the result as premultiplied RGBA bytes.
fn resize_premultiplied(src: &image::RgbaImage, px: u32) -> Vec<u8> {
    let mut pre = src.clone();
    for p in pre.pixels_mut() {
        let a = u32::from(p.0[3]);
        for c in &mut p.0[..3] {
            *c = ((u32::from(*c) * a + 127) / 255) as u8;
        }
    }
    resize(&pre, px, px, FilterType::Lanczos3).into_raw()
}

/// Window and taskbar icon; `None` (default OS icon) if the embedded PNG cannot be decoded.
pub fn window_icon() -> Option<egui::IconData> {
    let mut rgba = resize_premultiplied(&decode()?, WINDOW_ICON_PX);
    for p in rgba.chunks_exact_mut(4) {
        let a = u32::from(p[3]);
        if a > 0 && a < 255 {
            for c in &mut p[..3] {
                *c = ((u32::from(*c) * 255 / a).min(255)) as u8;
            }
        }
    }
    Some(egui::IconData {
        rgba,
        width: WINDOW_ICON_PX,
        height: WINDOW_ICON_PX,
    })
}

fn logo_texture(ctx: &Context) -> Option<TextureHandle> {
    let id = egui::Id::new("app_logo_texture");
    if let Some(tex) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return Some(tex);
    }
    let img = resize(
        &decode()?,
        LOGO_TEXTURE_PX,
        LOGO_TEXTURE_PX,
        FilterType::Lanczos3,
    );
    let size = [LOGO_TEXTURE_PX as usize; 2];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    let tex = ctx.load_texture("app_logo", color, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    Some(tex)
}

/// Paints the logo into `rect`; returns `false` (nothing drawn) if the image is unavailable.
pub fn paint_logo(ui: &Ui, rect: Rect) -> bool {
    let Some(tex) = logo_texture(ui.ctx()) else {
        return false;
    };
    let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    ui.painter().image(tex.id(), rect, uv, Color32::WHITE);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backdrop_is_transparent_and_icon_body_is_opaque() {
        let img = decode().expect("embedded icon decodes");
        let (w, h) = img.dimensions();
        assert_eq!(img.get_pixel(0, 0).0[3], 0, "corner must be transparent");
        assert_eq!(
            img.get_pixel(w / 2, h / 2).0[3],
            255,
            "centre must be opaque"
        );
    }

    #[test]
    fn window_icon_has_expected_size() {
        let icon = window_icon().expect("icon");
        assert_eq!(
            icon.rgba.len(),
            (WINDOW_ICON_PX * WINDOW_ICON_PX * 4) as usize
        );
    }
}
