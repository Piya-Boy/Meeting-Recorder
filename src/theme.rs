use egui::{Color32, Stroke};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    System,
}

#[derive(Debug, Clone, Copy)]
pub struct ThemeColors {
    // Mode-specific
    pub background: Color32,
    pub sidebar: Color32,
    pub surface: Color32,
    pub surface_elevated: Color32,
    pub on_background: Color32,
    pub on_surface: Color32,
    pub on_surface_variant: Color32,
    pub outline: Color32,
    pub outline_variant: Color32,
    pub primary: Color32,
    pub on_primary: Color32,
    pub primary_container: Color32,
    pub on_primary_container: Color32,
    pub scrim: Color32,

    // Mode-independent semantic colors
    pub brand_blue: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub on_warning: Color32,
    pub error: Color32,
    pub on_error: Color32,

    // Legible variants of the semantic colours for *text and icons* on light surfaces.
    // The bright base colours are ~2:1 against white; dark mode keeps the base values.
    pub success_text: Color32,
    pub warning_text: Color32,
    pub error_text: Color32,
    pub link_text: Color32,
}

impl ThemeColors {
    pub const DARK: Self = Self {
        background: Color32::from_rgb(0x11, 0x13, 0x18),
        sidebar: Color32::from_rgb(0x17, 0x1A, 0x21),
        surface: Color32::from_rgb(0x1D, 0x22, 0x2B),
        surface_elevated: Color32::from_rgb(0x24, 0x2A, 0x35),
        on_background: Color32::from_rgb(0xF1, 0xF5, 0xF9),
        on_surface: Color32::from_rgb(0xF1, 0xF5, 0xF9),
        on_surface_variant: Color32::from_rgb(0x94, 0xA3, 0xB8),
        outline: Color32::from_rgb(0x2E, 0x35, 0x43),
        outline_variant: Color32::from_rgb(0x1F, 0x24, 0x2E),
        primary: Color32::from_rgb(0x25, 0x63, 0xEB),
        on_primary: Color32::from_rgb(0xFF, 0xFF, 0xFF),
        primary_container: Color32::from_rgb(0x1E, 0x3A, 0x5F),
        on_primary_container: Color32::from_rgb(0xDB, 0xEA, 0xFE),
        scrim: Color32::from_black_alpha(180),

        brand_blue: Color32::from_rgb(0x3B, 0x82, 0xF6),
        success: Color32::from_rgb(0x22, 0xC5, 0x5E),
        warning: Color32::from_rgb(0xF5, 0x9E, 0x0B),
        on_warning: Color32::from_rgb(0x45, 0x1A, 0x03),
        error: Color32::from_rgb(0xEF, 0x44, 0x44),
        on_error: Color32::from_rgb(0x1C, 0x1C, 0x1C),

        success_text: Color32::from_rgb(0x22, 0xC5, 0x5E),
        warning_text: Color32::from_rgb(0xF5, 0x9E, 0x0B),
        error_text: Color32::from_rgb(0xEF, 0x44, 0x44),
        link_text: Color32::from_rgb(0x3B, 0x82, 0xF6),
    };

    pub const LIGHT: Self = Self {
        background: Color32::from_rgb(0xF8, 0xFA, 0xFC),
        sidebar: Color32::from_rgb(0xF1, 0xF5, 0xF9),
        surface: Color32::from_rgb(0xFF, 0xFF, 0xFF),
        surface_elevated: Color32::from_rgb(0xE2, 0xE8, 0xF0),
        on_background: Color32::from_rgb(0x1E, 0x29, 0x3B),
        on_surface: Color32::from_rgb(0x1E, 0x29, 0x3B),
        on_surface_variant: Color32::from_rgb(0x47, 0x55, 0x69),
        outline: Color32::from_rgb(0xCB, 0xD5, 0xE1),
        outline_variant: Color32::from_rgb(0xE2, 0xE8, 0xF0),
        primary: Color32::from_rgb(0x25, 0x63, 0xEB),
        on_primary: Color32::from_rgb(0xFF, 0xFF, 0xFF),
        primary_container: Color32::from_rgb(0xDB, 0xEA, 0xFE),
        on_primary_container: Color32::from_rgb(0x1E, 0x3A, 0x8A),
        scrim: Color32::from_rgba_premultiplied(9, 13, 25, 150),

        brand_blue: Color32::from_rgb(0x3B, 0x82, 0xF6),
        success: Color32::from_rgb(0x22, 0xC5, 0x5E),
        warning: Color32::from_rgb(0xF5, 0x9E, 0x0B),
        on_warning: Color32::from_rgb(0x45, 0x1A, 0x03),
        error: Color32::from_rgb(0xEF, 0x44, 0x44),
        on_error: Color32::from_rgb(0x1C, 0x1C, 0x1C),

        success_text: Color32::from_rgb(0x15, 0x80, 0x3D),
        warning_text: Color32::from_rgb(0xB4, 0x53, 0x09),
        error_text: Color32::from_rgb(0xB9, 0x1C, 0x1C),
        link_text: Color32::from_rgb(0x1D, 0x4E, 0xD8),
    };

    /// `system` is what Windows reports; dark when it says nothing.
    pub fn get(mode: ThemeMode, system: Option<egui::Theme>) -> Self {
        match mode {
            ThemeMode::Dark => Self::DARK,
            ThemeMode::Light => Self::LIGHT,
            ThemeMode::System => match system {
                Some(egui::Theme::Light) => Self::LIGHT,
                Some(egui::Theme::Dark) | None => Self::DARK,
            },
        }
    }

    pub fn is_light(&self) -> bool {
        self.background.r() >= 128
    }

    /// Re-tints the interactive colours with `accent` (index 0 = the default blue).
    pub fn with_accent(mut self, accent_idx: usize) -> Self {
        let Some((_, brand)) = crate::screens::settings::appearance::ACCENTS
            .get(accent_idx)
            .copied()
        else {
            return self;
        };
        if accent_idx == 0 {
            return self;
        }
        let dark = !self.is_light();
        let mix = |a: Color32, b: Color32, t: f32| {
            let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
            Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
        };
        self.primary = brand;
        self.brand_blue = brand;
        if dark {
            self.primary_container = mix(self.surface, brand, 0.32);
            self.on_primary_container = mix(brand, Color32::WHITE, 0.85);
        } else {
            self.primary_container = mix(Color32::WHITE, brand, 0.18);
            self.on_primary_container = mix(brand, Color32::BLACK, 0.55);
        }
        self
    }
}

/// Load a system font file into `fonts` under `name`. Returns true on success.
/// Embeds a bundled font file under `name`.
macro_rules! embed_font {
    ($fonts:expr, $name:expr, $file:expr) => {
        $fonts.font_data.insert(
            $name.to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!(concat!(
                "../assets/fonts/",
                $file
            )))),
        )
    };
}

/// Installs the design-system fonts, all bundled under `assets/fonts` (SIL OFL):
/// Inter (UI), JetBrains Mono (timer / paths / chips) and Noto Sans Thai (Thai glyphs).
/// Phosphor icons and egui's built-in fonts stay behind them as fallbacks.
///
/// Weights are separate families because egui has no variable-weight support. They sit one
/// step lighter than Stitch's CSS weights, which render heavier in egui than in a browser:
/// `Proportional` = 400, [`medium_family`] = 400 (buttons), [`bold_family`] = 500 (titles),
/// `Monospace` = 400, [`mono_bold_family`] = 500.
pub fn configure_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

    embed_font!(fonts, "Inter", "Inter-Regular.ttf");
    embed_font!(fonts, "InterMedium", "Inter-Medium.ttf");
    embed_font!(fonts, "Mono", "JetBrainsMono-Regular.ttf");
    embed_font!(fonts, "MonoBold", "JetBrainsMono-Medium.ttf");
    embed_font!(fonts, "Thai", "NotoSansThai-Regular.ttf");
    embed_font!(fonts, "ThaiMedium", "NotoSansThai-Medium.ttf");

    // Fallbacks already present in the default families (icons, emoji, egui defaults).
    let prop_tail = fonts.families[&egui::FontFamily::Proportional].clone();
    let mono_tail = fonts.families[&egui::FontFamily::Monospace].clone();

    // Phosphor icons live in the Private Use Area, where Noto Sans Thai maps unrelated
    // glyphs. Order: Latin face -> icons -> Thai, so neither text nor icons are shadowed.
    let chain = |head: &[&str], tail: &[String]| -> Vec<String> {
        let (latin, thai) = head.split_at(1);
        latin
            .iter()
            .map(|n| (*n).to_owned())
            .chain(std::iter::once("phosphor".to_owned()))
            .chain(thai.iter().map(|n| (*n).to_owned()))
            .chain(tail.iter().filter(|n| *n != "phosphor").cloned())
            .collect()
    };

    fonts.families.insert(
        egui::FontFamily::Proportional,
        chain(&["Inter", "Thai"], &prop_tail),
    );
    fonts.families.insert(
        egui::FontFamily::Monospace,
        chain(&["Mono", "Thai"], &mono_tail),
    );
    fonts
        .families
        .insert(medium_family(), chain(&["Inter", "Thai"], &prop_tail));
    fonts.families.insert(
        bold_family(),
        chain(&["InterMedium", "ThaiMedium"], &prop_tail),
    );
    fonts.families.insert(
        mono_bold_family(),
        chain(&["MonoBold", "ThaiMedium"], &mono_tail),
    );

    ctx.set_fonts(fonts);
}

/// Medium-weight proportional family (button and label text).
pub fn medium_family() -> egui::FontFamily {
    egui::FontFamily::Name("Medium".into())
}

/// Bold proportional family (see [`configure_fonts`]).
pub fn bold_family() -> egui::FontFamily {
    egui::FontFamily::Name("Bold".into())
}

/// Bold monospace family (see [`configure_fonts`]).
pub fn mono_bold_family() -> egui::FontFamily {
    egui::FontFamily::Name("MonoBold".into())
}

pub fn apply_theme(ctx: &egui::Context, theme: &ThemeColors) {
    let mut visuals = if theme.is_light() {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };

    visuals.override_text_color = Some(theme.on_background);
    visuals.panel_fill = theme.background;
    visuals.window_fill = theme.surface;
    visuals.faint_bg_color = theme.surface_elevated;
    visuals.extreme_bg_color = theme.background;
    visuals.window_stroke = Stroke::new(1.0, theme.outline);

    visuals.widgets.noninteractive.bg_fill = theme.surface;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, theme.outline);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, theme.on_surface);

    visuals.widgets.inactive.bg_fill = theme.surface_elevated;
    visuals.widgets.inactive.weak_bg_fill = theme.background;
    visuals.widgets.hovered.weak_bg_fill = theme.surface_elevated;
    visuals.widgets.active.weak_bg_fill = theme.primary_container;
    let radius = egui::CornerRadius::same(6);
    visuals.widgets.noninteractive.corner_radius = radius;
    visuals.widgets.inactive.corner_radius = radius;
    visuals.widgets.hovered.corner_radius = radius;
    visuals.widgets.active.corner_radius = radius;
    visuals.widgets.open.corner_radius = radius;
    visuals.widgets.open.weak_bg_fill = theme.background;
    visuals.window_corner_radius = egui::CornerRadius::same(10);
    visuals.menu_corner_radius = egui::CornerRadius::same(6);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, theme.outline);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, theme.on_surface_variant);

    visuals.widgets.hovered.bg_fill = theme.surface_elevated;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, theme.brand_blue);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, theme.on_surface);

    visuals.widgets.active.bg_fill = theme.primary_container;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, theme.brand_blue);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, theme.on_primary_container);

    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(0x3B, 0x82, 0xF6, 76);
    visuals.selection.stroke = Stroke::new(1.0, theme.brand_blue);

    ctx.set_visuals(visuals);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_system_theme_follows_windows() {
        let colours = |mode, system| ThemeColors::get(mode, system).background;
        let (dark, light) = (ThemeColors::DARK.background, ThemeColors::LIGHT.background);
        assert_eq!(colours(ThemeMode::System, Some(egui::Theme::Light)), light);
        assert_eq!(colours(ThemeMode::System, Some(egui::Theme::Dark)), dark);
        // Windows did not say.
        assert_eq!(colours(ThemeMode::System, None), dark);
        // A chosen theme ignores Windows.
        assert_eq!(colours(ThemeMode::Dark, Some(egui::Theme::Light)), dark);
        assert_eq!(colours(ThemeMode::Light, Some(egui::Theme::Dark)), light);
    }

    #[test]
    fn default_accent_leaves_palette_untouched() {
        let base = ThemeColors::DARK;
        assert_eq!(base.with_accent(0).primary, base.primary);
    }

    #[test]
    fn non_default_accent_retints_primary() {
        let base = ThemeColors::DARK;
        let green = base.with_accent(1);
        assert_ne!(green.primary, base.primary);
        assert_ne!(green.primary_container, base.primary_container);
    }

    #[test]
    fn out_of_range_accent_is_ignored() {
        let base = ThemeColors::LIGHT;
        assert_eq!(base.with_accent(99).primary, base.primary);
    }
}
