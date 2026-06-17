use eframe::egui::{
    self, Color32, FontData, FontDefinitions, FontFamily, FontId, TextStyle, Visuals,
};

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
    apply(ctx);
}

pub fn apply(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = BG;
    visuals.faint_bg_color = PANEL_SOFT;
    visuals.widgets.noninteractive.bg_fill = PANEL;
    visuals.widgets.inactive.bg_fill = PANEL_SOFT;
    visuals.widgets.inactive.weak_bg_fill = PANEL_SOFT;
    visuals.widgets.inactive.fg_stroke.color = TEXT;
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(43, 49, 64);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(43, 49, 64);
    visuals.widgets.hovered.fg_stroke.color = TEXT;
    visuals.widgets.active.bg_fill = Color32::from_rgb(50, 59, 75);
    visuals.widgets.active.weak_bg_fill = Color32::from_rgb(50, 59, 75);
    visuals.widgets.active.fg_stroke.color = TEXT;
    visuals.selection.bg_fill = Color32::from_rgb(41, 108, 100);
    visuals.hyperlink_color = ACCENT;
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
