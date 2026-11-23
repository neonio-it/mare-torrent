//! Peças visuais pequenas reaproveitadas pela interface.

use eframe::egui::{
    self, Align2, Button, Color32, CornerRadius, FontId, Pos2, Rect, Response, RichText, Sense,
    Stroke, Ui, pos2, vec2,
};

use crate::theme::{Palette, semibold};

/// Barra de progresso fina e arredondada.
pub fn progress(ui: &mut Ui, frac: f32, color: Color32, track: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 6.0), Sense::hover());
    let r = CornerRadius::same(3);
    ui.painter().rect_filled(rect, r, track);
    let frac = frac.clamp(0.0, 1.0);
    if frac > 0.0 {
        let w = (rect.width() * frac).max(6.0);
        let fill = Rect::from_min_size(rect.min, vec2(w, rect.height()));
        ui.painter().rect_filled(fill, r, color);
    }
}

/// Selo colorido com o estado do torrent.
pub fn badge(ui: &mut Ui, text: &str, fg: Color32, bg: Color32) -> Response {
    let font = FontId::proportional(11.5);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, fg);
    let size = galley.size() + vec2(16.0, 6.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(10), bg);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, fg);
    resp
}

/// Botão principal (cor de destaque).
pub fn primary(ui: &mut Ui, p: &Palette, text: &str) -> Response {
    ui.add(
        Button::new(RichText::new(text).color(p.on_accent).font(semibold(14.0)))
            .fill(p.accent)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(8))
            .min_size(vec2(0.0, 32.0)),
    )
}

/// Botão vermelho para ações destrutivas.
pub fn danger(ui: &mut Ui, p: &Palette, text: &str) -> Response {
    ui.add(
        Button::new(RichText::new(text).color(Color32::WHITE).font(semibold(14.0)))
            .fill(p.danger)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(8))
            .min_size(vec2(0.0, 32.0)),
    )
}

/// Botão secundário (contorno).
pub fn secondary(ui: &mut Ui, text: &str) -> Response {
    ui.add(Button::new(text).corner_radius(CornerRadius::same(8)).min_size(vec2(0.0, 32.0)))
}

/// Botão só com ícone, sem moldura até passar o mouse.
pub fn icon(ui: &mut Ui, p: &Palette, glyph: &str, tip: &str) -> Response {
    ui.add(
        Button::new(RichText::new(glyph).size(16.0).color(p.muted))
            .frame_when_inactive(false)
            .corner_radius(CornerRadius::same(8))
            .min_size(vec2(32.0, 30.0)),
    )
    .on_hover_text(tip)
}

/// Logotipo: quadradinho com ondas + o nome do app.
pub fn logo(ui: &mut Ui, p: &Palette) {
    let (rect, _) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::hover());
    paint_mark(ui.painter(), rect, p.accent, if p.dark { p.bg } else { Color32::WHITE });
    ui.add_space(2.0);
    ui.label(RichText::new("Maré").font(semibold(18.0)).color(p.text));
}

pub fn paint_mark(painter: &egui::Painter, rect: Rect, bg: Color32, fg: Color32) {
    painter.rect_filled(rect, CornerRadius::same((rect.width() * 0.28) as u8), bg);
    let w = rect.width();
    for (i, dy) in [-0.16f32, 0.12].iter().enumerate() {
        let pts: Vec<Pos2> = (0..=24)
            .map(|k| {
                let t = k as f32 / 24.0;
                let x = rect.left() + w * (0.18 + 0.64 * t);
                let phase = if i == 0 { 0.0 } else { 0.9 };
                let y = rect.center().y + w * dy + (t * std::f32::consts::TAU + phase).sin() * w * 0.07;
                pos2(x, y)
            })
            .collect();
        painter.add(egui::Shape::line(pts, Stroke::new(w * 0.085, fg)));
    }
}

/// Item clicável da barra lateral (filtro), com contagem à direita.
pub fn nav_item(ui: &mut Ui, p: &Palette, glyph: &str, label: &str, count: usize, selected: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, CornerRadius::same(8), p.accent_soft);
    } else if resp.hovered() {
        painter.rect_filled(rect, CornerRadius::same(8), p.card_hover);
    }
    let fg = if selected { p.accent } else { p.muted };
    let txt = if selected { p.text } else { p.muted };
    painter.text(pos2(rect.left() + 12.0, rect.center().y), Align2::LEFT_CENTER, glyph, FontId::proportional(15.0), fg);
    painter.text(
        pos2(rect.left() + 36.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        if selected { semibold(14.0) } else { FontId::proportional(14.0) },
        txt,
    );
    if count > 0 {
        painter.text(
            pos2(rect.right() - 12.0, rect.center().y),
            Align2::RIGHT_CENTER,
            count.to_string(),
            FontId::proportional(12.5),
            if selected { p.accent } else { p.faint },
        );
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Título pequeno de seção.
pub fn section(ui: &mut Ui, p: &Palette, text: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(text).size(11.5).color(p.faint));
}

/// Moldura padrão dos cartões.
pub fn card_frame(p: &Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(p.card)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::symmetric(16, 12))
}
