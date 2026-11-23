//! Paleta e estilo visual do Maré (tema escuro e claro).

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow,
    Stroke, TextStyle, Visuals,
};

use crate::engine::Status;

#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub panel: Color32,
    pub card: Color32,
    pub card_hover: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub on_accent: Color32,
    pub success: Color32,
    pub success_soft: Color32,
    pub warning: Color32,
    pub warning_soft: Color32,
    pub danger: Color32,
    pub danger_soft: Color32,
    pub track: Color32,
}

impl Palette {
    pub fn dark() -> Self {
        Self {
            dark: true,
            bg: Color32::from_rgb(0x0E, 0x13, 0x1A),
            panel: Color32::from_rgb(0x12, 0x18, 0x21),
            card: Color32::from_rgb(0x18, 0x20, 0x2B),
            card_hover: Color32::from_rgb(0x1D, 0x27, 0x34),
            border: Color32::from_rgb(0x25, 0x30, 0x3E),
            text: Color32::from_rgb(0xE6, 0xED, 0xF3),
            muted: Color32::from_rgb(0x8B, 0x98, 0xA8),
            faint: Color32::from_rgb(0x5C, 0x69, 0x7A),
            accent: Color32::from_rgb(0x4C, 0x9E, 0xFF),
            accent_soft: Color32::from_rgb(0x16, 0x2C, 0x48),
            on_accent: Color32::from_rgb(0x05, 0x10, 0x1F),
            success: Color32::from_rgb(0x2F, 0xC9, 0xA4),
            success_soft: Color32::from_rgb(0x10, 0x33, 0x2D),
            warning: Color32::from_rgb(0xE8, 0xB0, 0x4B),
            warning_soft: Color32::from_rgb(0x3A, 0x2C, 0x12),
            danger: Color32::from_rgb(0xF2, 0x6D, 0x6D),
            danger_soft: Color32::from_rgb(0x3D, 0x18, 0x1B),
            track: Color32::from_rgb(0x24, 0x2E, 0x3B),
        }
    }

    pub fn light() -> Self {
        Self {
            dark: false,
            bg: Color32::from_rgb(0xF3, 0xF6, 0xF9),
            panel: Color32::from_rgb(0xFA, 0xFB, 0xFC),
            card: Color32::WHITE,
            card_hover: Color32::from_rgb(0xF7, 0xF9, 0xFB),
            border: Color32::from_rgb(0xDF, 0xE5, 0xEC),
            text: Color32::from_rgb(0x1B, 0x22, 0x2C),
            muted: Color32::from_rgb(0x5E, 0x6B, 0x7A),
            faint: Color32::from_rgb(0x94, 0x9F, 0xAC),
            accent: Color32::from_rgb(0x1A, 0x6F, 0xE0),
            accent_soft: Color32::from_rgb(0xE3, 0xEE, 0xFD),
            on_accent: Color32::WHITE,
            success: Color32::from_rgb(0x0E, 0x8F, 0x70),
            success_soft: Color32::from_rgb(0xDD, 0xF4, 0xEE),
            warning: Color32::from_rgb(0xA8, 0x6A, 0x05),
            warning_soft: Color32::from_rgb(0xFB, 0xF0, 0xD9),
            danger: Color32::from_rgb(0xC9, 0x3B, 0x3B),
            danger_soft: Color32::from_rgb(0xFB, 0xE6, 0xE6),
            track: Color32::from_rgb(0xE4, 0xE9, 0xEF),
        }
    }

    /// Cor forte e cor de fundo do selo de cada estado.
    pub fn status(&self, s: Status) -> (Color32, Color32) {
        match s {
            Status::Downloading => (self.accent, self.accent_soft),
            Status::Seeding | Status::Done => (self.success, self.success_soft),
            Status::Paused => (self.warning, self.warning_soft),
            Status::Checking => (self.muted, self.track),
            Status::Error => (self.danger, self.danger_soft),
        }
    }
}

pub fn apply(ctx: &egui::Context, p: &Palette) {
    let mut v = if p.dark { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = p.bg;
    v.window_fill = p.card;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(14);
    v.window_shadow = Shadow {
        offset: [0, 8],
        blur: 28,
        spread: 0,
        color: Color32::from_black_alpha(if p.dark { 140 } else { 40 }),
    };
    v.menu_corner_radius = CornerRadius::same(10);
    v.extreme_bg_color = if p.dark { p.panel } else { p.card };
    v.text_edit_bg_color = Some(if p.dark { p.panel } else { p.card });
    v.faint_bg_color = p.card_hover;
    v.hyperlink_color = p.accent;
    v.slider_trailing_fill = true;
    v.override_text_color = Some(p.text);
    v.selection.bg_fill = p.accent.gamma_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0, p.accent);

    let r = CornerRadius::same(8);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = r;
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.inactive.weak_bg_fill = p.card;
    v.widgets.inactive.bg_fill = p.track;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.hovered.weak_bg_fill = p.card_hover;
    v.widgets.hovered.bg_fill = p.border;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.faint);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.active.weak_bg_fill = p.border;
    v.widgets.active.bg_fill = p.accent;
    v.widgets.active.bg_stroke = Stroke::new(1.0, p.accent);
    v.widgets.active.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.open.weak_bg_fill = p.card_hover;

    let theme = if p.dark { egui::Theme::Dark } else { egui::Theme::Light };
    ctx.set_theme(theme);
    ctx.set_visuals_of(theme, v);

    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        style.spacing.interact_size.y = 30.0;
        style.spacing.window_margin = Margin::same(20);
        style.spacing.scroll.bar_width = 8.0;
        style.spacing.scroll.floating = true;
        style.text_styles = [
            (TextStyle::Small, FontId::proportional(12.0)),
            (TextStyle::Body, FontId::proportional(14.0)),
            (TextStyle::Button, FontId::proportional(14.0)),
            (TextStyle::Heading, FontId::new(20.0, FontFamily::Name("semibold".into()))),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ]
        .into();
    });
}

/// Usa a fonte do Windows (Segoe UI) quando ela existe; senão fica com a padrão do egui.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let fonts_dir = std::env::var_os("WINDIR")
        .map(|w| std::path::PathBuf::from(w).join("Fonts"))
        .unwrap_or_else(|| "C:\\Windows\\Fonts".into());
    let load = |file: &str| std::fs::read(fonts_dir.join(file)).ok();

    let default_prop = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();

    for (name, file) in [("segoe", "segoeui.ttf"), ("segoe-sb", "seguisb.ttf"), ("segoe-sym", "seguisym.ttf")] {
        if let Some(b) = load(file) {
            fonts.font_data.insert(name.into(), Arc::new(FontData::from_owned(b)));
        }
    }
    let has = |n: &str| fonts.font_data.contains_key(n);

    // Texto normal: Segoe UI, depois as fontes do egui, e os símbolos do Windows no fim.
    let mut prop = Vec::new();
    if has("segoe") {
        prop.push("segoe".to_string());
    }
    prop.extend(default_prop.iter().cloned());
    if has("segoe-sym") {
        prop.push("segoe-sym".into());
    }

    // Títulos: Segoe UI Semibold (ou a normal, se faltar).
    let mut sb = Vec::new();
    if has("segoe-sb") {
        sb.push("segoe-sb".to_string());
    } else if has("segoe") {
        sb.push("segoe".to_string());
    }
    sb.extend(default_prop.iter().cloned());
    if has("segoe-sym") {
        sb.push("segoe-sym".into());
    }

    fonts.families.insert(FontFamily::Proportional, prop);
    fonts.families.insert(FontFamily::Name("semibold".into()), sb);

    ctx.set_fonts(fonts);
}

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}
