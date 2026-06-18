use std::sync::atomic::{AtomicU8, Ordering};

use eframe::egui::{
    self, Color32, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle, Visuals,
};

static THEME_MODE: AtomicU8 = AtomicU8::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemeMode {
    Dark,
    Light,
}

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub bg: Color32,
    pub panel: Color32,
    pub panel_soft: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_deep: Color32,
    pub accent_soft: Color32,
    pub accent_shadow: Color32,
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
    apply(ctx, ThemeMode::Dark);
}

pub fn apply(ctx: &egui::Context, mode: ThemeMode) {
    THEME_MODE.store(
        match mode {
            ThemeMode::Dark => 0,
            ThemeMode::Light => 1,
        },
        Ordering::Relaxed,
    );
    let palette = palette(mode);
    let mut visuals = match mode {
        ThemeMode::Dark => Visuals::dark(),
        ThemeMode::Light => Visuals::light(),
    };
    visuals.panel_fill = palette.bg;
    visuals.window_fill = palette.panel;
    visuals.window_stroke = Stroke::NONE;
    visuals.extreme_bg_color = palette.bg;
    visuals.faint_bg_color = palette.panel_soft;
    visuals.widgets.noninteractive.bg_fill = palette.panel;
    visuals.widgets.noninteractive.bg_stroke = Stroke::NONE;
    visuals.widgets.inactive.bg_fill = palette.panel_soft;
    visuals.widgets.inactive.bg_stroke = Stroke::NONE;
    visuals.widgets.inactive.weak_bg_fill = palette.panel_soft;
    visuals.widgets.inactive.fg_stroke.color = palette.text;
    visuals.widgets.hovered.bg_fill = if mode == ThemeMode::Dark {
        Color32::from_rgb(43, 49, 64)
    } else {
        palette.accent_soft
    };
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
    visuals.hyperlink_color = palette.accent;
    let mut style = (*ctx.style()).clone();
    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 7.0);
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

pub fn palette(mode: ThemeMode) -> Palette {
    match mode {
        ThemeMode::Dark => Palette {
            bg: BG,
            panel: PANEL,
            panel_soft: PANEL_SOFT,
            text: TEXT,
            muted: MUTED,
            accent: ACCENT,
            accent_deep: Color32::from_rgb(34, 75, 82),
            accent_soft: Color32::from_rgb(27, 43, 46),
            accent_shadow: Color32::from_rgba_unmultiplied(69, 238, 216, 28),
            info: Color32::from_rgb(120, 164, 255),
            warning: WARNING,
        },
        ThemeMode::Light => Palette {
            bg: Color32::from_rgb(238, 241, 245),
            panel: Color32::from_rgb(250, 251, 253),
            panel_soft: Color32::from_rgb(229, 236, 242),
            text: Color32::from_rgb(32, 39, 50),
            muted: Color32::from_rgb(101, 112, 130),
            accent: Color32::from_rgb(0, 118, 137),
            accent_deep: Color32::from_rgb(23, 94, 102),
            accent_soft: Color32::from_rgb(225, 240, 239),
            accent_shadow: Color32::from_rgba_unmultiplied(69, 238, 216, 38),
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

pub fn info() -> Color32 {
    palette(current_mode()).info
}

pub fn warning() -> Color32 {
    palette(current_mode()).warning
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
