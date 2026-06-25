use std::sync::atomic::{AtomicU8, Ordering};

use eframe::egui::{
    self, Color32, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle, Visuals,
};

static THEME_MODE: AtomicU8 = AtomicU8::new(0);
static THEME_ACCENT: AtomicU8 = AtomicU8::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemeMode {
    Dark,
    Light,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemeAccent {
    Green,
    Blue,
    Purple,
    Rose,
    Orange,
}

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub bg: Color32,
    pub panel: Color32,
    pub panel_soft: Color32,
    pub panel_recessed: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_deep: Color32,
    pub accent_soft: Color32,
    pub accent_shadow: Color32,
    pub hover: Color32,
    pub scroll_track: Color32,
    pub inset_highlight: Color32,
    pub inset_shadow: Color32,
    pub info: Color32,
    pub warning: Color32,
}

pub const BG: Color32 = Color32::from_rgb(16, 18, 24);
pub const PANEL: Color32 = Color32::from_rgb(24, 27, 36);
pub const PANEL_SOFT: Color32 = Color32::from_rgb(31, 35, 46);
pub const TEXT: Color32 = Color32::from_rgb(235, 239, 246);
pub const MUTED: Color32 = Color32::from_rgb(142, 151, 169);
pub const ACCENT: Color32 = Color32::from_rgb(85, 195, 176);
pub const WARNING: Color32 = Color32::from_rgb(242, 171, 90);

pub const LANES: [Color32; 8] = [
    Color32::from_rgb(85, 195, 176),
    Color32::from_rgb(244, 113, 116),
    Color32::from_rgb(120, 164, 255),
    Color32::from_rgb(232, 190, 95),
    Color32::from_rgb(177, 136, 255),
    Color32::from_rgb(104, 210, 121),
    Color32::from_rgb(247, 142, 214),
    Color32::from_rgb(107, 202, 231),
];

pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);
    apply(ctx, ThemeMode::Dark, ThemeAccent::Green);
}

pub fn apply(ctx: &egui::Context, mode: ThemeMode, accent: ThemeAccent) {
    THEME_MODE.store(
        match mode {
            ThemeMode::Dark => 0,
            ThemeMode::Light => 1,
        },
        Ordering::Relaxed,
    );
    THEME_ACCENT.store(accent_index(accent), Ordering::Relaxed);
    let palette = palette_for(mode, accent);
    let mut visuals = match mode {
        ThemeMode::Dark => Visuals::dark(),
        ThemeMode::Light => Visuals::light(),
    };
    visuals.panel_fill = palette.bg;
    visuals.window_fill = palette.panel;
    visuals.window_stroke = Stroke::NONE;
    visuals.extreme_bg_color = palette.scroll_track;
    visuals.faint_bg_color = palette.panel_soft;
    visuals.widgets.noninteractive.bg_fill = palette.panel;
    visuals.widgets.noninteractive.bg_stroke = Stroke::NONE;
    visuals.widgets.inactive.bg_fill = palette.panel_soft;
    visuals.widgets.inactive.bg_stroke = Stroke::NONE;
    visuals.widgets.inactive.weak_bg_fill = palette.panel_soft;
    visuals.widgets.inactive.fg_stroke.color = palette.text;
    visuals.widgets.hovered.bg_fill = palette.hover;
    visuals.widgets.hovered.bg_stroke = Stroke::NONE;
    visuals.widgets.hovered.weak_bg_fill = visuals.widgets.hovered.bg_fill;
    visuals.widgets.hovered.fg_stroke.color = palette.text;
    visuals.widgets.active.bg_fill = if mode == ThemeMode::Dark {
        palette.accent_deep
    } else {
        palette.accent_deep
    };
    visuals.widgets.active.bg_stroke = Stroke::NONE;
    visuals.widgets.active.weak_bg_fill = visuals.widgets.active.bg_fill;
    visuals.widgets.active.fg_stroke.color = Color32::WHITE;
    visuals.widgets.open.bg_stroke = Stroke::NONE;
    visuals.selection.bg_fill = if mode == ThemeMode::Dark {
        palette.accent_deep
    } else {
        palette.accent_deep
    };
    visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.hyperlink_color = palette.accent;
    let mut style = (*ctx.style()).clone();
    style.visuals = visuals;
    style.spacing.scroll.foreground_color = false;
    style.spacing.scroll.active_background_opacity = 0.24;
    style.spacing.scroll.interact_background_opacity = 0.36;
    style.spacing.scroll.active_handle_opacity = 0.74;
    style.spacing.scroll.interact_handle_opacity = 1.0;
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 7.0);
    style.interaction.tooltip_delay = 0.12;
    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(26.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(14.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]
    .into();
    ctx.set_style(style);
}

pub fn current_mode() -> ThemeMode {
    if THEME_MODE.load(Ordering::Relaxed) == 1 {
        ThemeMode::Light
    } else {
        ThemeMode::Dark
    }
}

pub fn current_accent() -> ThemeAccent {
    accent_from_index(THEME_ACCENT.load(Ordering::Relaxed))
}

pub fn palette(mode: ThemeMode) -> Palette {
    palette_for(mode, current_accent())
}

pub fn palette_for(mode: ThemeMode, accent: ThemeAccent) -> Palette {
    let seed = accent_seed(accent);
    let hsl = rgb_to_hsl(seed);
    let neutral_s = (hsl.s * 0.10).clamp(0.02, 0.09);
    let muted_s = (hsl.s * 0.18).clamp(0.04, 0.16);
    let neutral = |lightness: f32| hsl_to_rgb(hsl.h, neutral_s, lightness);
    let muted_neutral = |lightness: f32| hsl_to_rgb(hsl.h, muted_s, lightness);
    let recessed_neutral =
        |lightness: f32| hsl_to_rgb(hsl.h, (hsl.s * 0.20).clamp(0.08, 0.18), lightness);
    let accent_color = hsl_to_rgb(hsl.h, hsl.s, hsl.l);
    let accent_deep = match mode {
        ThemeMode::Dark => hsl_to_rgb(hsl.h, (hsl.s * 0.72).clamp(0.0, 1.0), 0.28),
        ThemeMode::Light => hsl_to_rgb(hsl.h, (hsl.s * 0.78).clamp(0.0, 1.0), 0.34),
    };
    let accent_soft = match mode {
        ThemeMode::Dark => hsl_to_rgb(hsl.h, (hsl.s * 0.42).clamp(0.0, 1.0), 0.16),
        ThemeMode::Light => hsl_to_rgb(hsl.h, (hsl.s * 0.30).clamp(0.0, 1.0), 0.91),
    };
    let hover = match mode {
        ThemeMode::Dark => hsl_to_rgb(hsl.h, (hsl.s * 0.36).clamp(0.0, 1.0), 0.22),
        ThemeMode::Light => hsl_to_rgb(hsl.h, (hsl.s * 0.30).clamp(0.0, 1.0), 0.88),
    };
    let scroll_track = match mode {
        ThemeMode::Dark => hsl_to_rgb(hsl.h, (hsl.s * 0.48).clamp(0.0, 1.0), 0.11),
        ThemeMode::Light => hsl_to_rgb(hsl.h, (hsl.s * 0.24).clamp(0.0, 1.0), 0.84),
    };
    let shadow_base = muted_neutral(match mode {
        ThemeMode::Dark => 0.36,
        ThemeMode::Light => 0.28,
    });
    let accent_shadow = match mode {
        ThemeMode::Dark => {
            Color32::from_rgba_unmultiplied(shadow_base.r(), shadow_base.g(), shadow_base.b(), 42)
        }
        ThemeMode::Light => {
            Color32::from_rgba_unmultiplied(shadow_base.r(), shadow_base.g(), shadow_base.b(), 58)
        }
    };
    match mode {
        ThemeMode::Dark => Palette {
            bg: neutral(0.085),
            panel: neutral(0.115),
            panel_soft: neutral(0.155),
            panel_recessed: neutral(0.125),
            text: muted_neutral(0.94),
            muted: muted_neutral(0.62),
            accent: accent_color,
            accent_deep,
            accent_soft,
            accent_shadow,
            hover,
            scroll_track,
            inset_highlight: Color32::from_rgba_unmultiplied(255, 255, 255, 22),
            inset_shadow: Color32::from_rgba_unmultiplied(
                shadow_base.r(),
                shadow_base.g(),
                shadow_base.b(),
                98,
            ),
            info: Color32::from_rgb(120, 164, 255),
            warning: WARNING,
        },
        ThemeMode::Light => Palette {
            bg: neutral(0.948),
            panel: neutral(0.982),
            panel_soft: neutral(0.91),
            panel_recessed: recessed_neutral(0.985),
            text: muted_neutral(0.16),
            muted: muted_neutral(0.46),
            accent: accent_color,
            accent_deep,
            accent_soft,
            accent_shadow,
            hover,
            scroll_track,
            inset_highlight: Color32::from_rgba_unmultiplied(255, 255, 255, 190),
            inset_shadow: Color32::from_rgba_unmultiplied(
                shadow_base.r(),
                shadow_base.g(),
                shadow_base.b(),
                86,
            ),
            info: Color32::from_rgb(59, 107, 185),
            warning: Color32::from_rgb(181, 98, 28),
        },
    }
}

pub fn bg() -> Color32 {
    palette(current_mode()).bg
}

pub fn panel() -> Color32 {
    palette(current_mode()).panel
}

pub fn panel_soft() -> Color32 {
    palette(current_mode()).panel_soft
}

pub fn panel_recessed() -> Color32 {
    palette(current_mode()).panel_recessed
}

pub fn text() -> Color32 {
    palette(current_mode()).text
}

pub fn muted() -> Color32 {
    palette(current_mode()).muted
}

pub fn accent() -> Color32 {
    palette(current_mode()).accent
}

pub fn accent_deep() -> Color32 {
    palette(current_mode()).accent_deep
}

pub fn accent_soft() -> Color32 {
    palette(current_mode()).accent_soft
}

pub fn accent_shadow() -> Color32 {
    palette(current_mode()).accent_shadow
}

pub fn hover() -> Color32 {
    palette(current_mode()).hover
}

pub fn inset_highlight() -> Color32 {
    palette(current_mode()).inset_highlight
}

pub fn inset_shadow() -> Color32 {
    palette(current_mode()).inset_shadow
}

pub fn info() -> Color32 {
    palette(current_mode()).info
}

pub fn warning() -> Color32 {
    palette(current_mode()).warning
}

pub fn all_accents() -> [ThemeAccent; 5] {
    [
        ThemeAccent::Green,
        ThemeAccent::Blue,
        ThemeAccent::Purple,
        ThemeAccent::Rose,
        ThemeAccent::Orange,
    ]
}

pub fn accent_color(accent: ThemeAccent) -> Color32 {
    accent_seed(accent)
}

fn accent_index(accent: ThemeAccent) -> u8 {
    match accent {
        ThemeAccent::Green => 0,
        ThemeAccent::Blue => 1,
        ThemeAccent::Purple => 2,
        ThemeAccent::Rose => 3,
        ThemeAccent::Orange => 4,
    }
}

fn accent_from_index(index: u8) -> ThemeAccent {
    match index {
        1 => ThemeAccent::Blue,
        2 => ThemeAccent::Purple,
        3 => ThemeAccent::Rose,
        4 => ThemeAccent::Orange,
        _ => ThemeAccent::Green,
    }
}

fn accent_seed(accent: ThemeAccent) -> Color32 {
    match accent {
        ThemeAccent::Green => ACCENT,
        ThemeAccent::Blue => Color32::from_rgb(74, 137, 229),
        ThemeAccent::Purple => Color32::from_rgb(142, 105, 222),
        ThemeAccent::Rose => Color32::from_rgb(210, 88, 132),
        ThemeAccent::Orange => Color32::from_rgb(213, 126, 48),
    }
}

#[derive(Clone, Copy, Debug)]
struct Hsl {
    h: f32,
    s: f32,
    l: f32,
}

fn rgb_to_hsl(color: Color32) -> Hsl {
    let r = color.r() as f32 / 255.0;
    let g = color.g() as f32 / 255.0;
    let b = color.b() as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) * 0.5;
    if (max - min).abs() < f32::EPSILON {
        return Hsl { h: 0.0, s: 0.0, l };
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let mut h = if (max - r).abs() < f32::EPSILON {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if (max - g).abs() < f32::EPSILON {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    h /= 6.0;
    Hsl { h, s, l }
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> Color32 {
    if s <= 0.0 {
        let v = (l.clamp(0.0, 1.0) * 255.0).round() as u8;
        return Color32::from_rgb(v, v, v);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let r = hue_to_rgb(p, q, h + 1.0 / 3.0);
    let g = hue_to_rgb(p, q, h);
    let b = hue_to_rgb(p, q, h - 1.0 / 3.0);
    Color32::from_rgb(
        (r.clamp(0.0, 1.0) * 255.0).round() as u8,
        (g.clamp(0.0, 1.0) * 255.0).round() as u8,
        (b.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}

fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        p + (q - p) * 6.0 * t
    } else if t < 1.0 / 2.0 {
        q
    } else if t < 2.0 / 3.0 {
        p + (q - p) * (2.0 / 3.0 - t) * 6.0
    } else {
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_surfaces_are_derived_from_theme_accent_hsl() {
        let green = palette_for(ThemeMode::Light, ThemeAccent::Green);
        let blue = palette_for(ThemeMode::Light, ThemeAccent::Blue);
        let dark_green = palette_for(ThemeMode::Dark, ThemeAccent::Green);
        let dark_blue = palette_for(ThemeMode::Dark, ThemeAccent::Blue);

        assert_ne!(green.bg, blue.bg);
        assert_ne!(green.panel, blue.panel);
        assert_ne!(green.panel_soft, blue.panel_soft);
        assert_ne!(green.panel_recessed, blue.panel_recessed);
        assert_ne!(green.accent_shadow, blue.accent_shadow);
        assert!(green.panel_recessed.r() >= green.panel.r());
        assert!(green.panel_recessed.g() >= green.panel.g());
        assert!(green.panel_recessed.b() >= green.panel.b());
        assert_ne!(dark_green.panel, dark_blue.panel);
        assert_ne!(dark_green.panel_recessed, dark_blue.panel_recessed);
    }
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    if let Some((name, bytes)) = load_system_cjk_font() {
        fonts
            .font_data
            .insert(name.clone(), FontData::from_owned(bytes).into());

        for family in [FontFamily::Proportional, FontFamily::Monospace] {
            fonts
                .families
                .entry(family)
                .or_default()
                .insert(0, name.clone());
        }
    }

    ctx.set_fonts(fonts);
}

fn load_system_cjk_font() -> Option<(String, Vec<u8>)> {
    [
        ("noto_sans_sc", r"C:\Windows\Fonts\NotoSansSC-VF.ttf"),
        ("microsoft_yahei", r"C:\Windows\Fonts\msyh.ttc"),
        ("simhei", r"C:\Windows\Fonts\simhei.ttf"),
        ("simsun", r"C:\Windows\Fonts\simsun.ttc"),
    ]
    .into_iter()
    .find_map(|(name, path)| {
        std::fs::read(path)
            .ok()
            .map(|bytes| (name.to_owned(), bytes))
    })
}
