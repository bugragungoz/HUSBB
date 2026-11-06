//! Bugra components drawn with egui: pill buttons, icon buttons, chips, switch, card, stepper.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2, vec2,
};

use super::icons::{self, Icon};
use super::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Outline,
    Primary,
}

fn font_button() -> FontId {
    FontId::new(13.5, theme::medium())
}

/// Pill button with an optional leading icon.
pub fn button(ui: &mut Ui, icon: Option<Icon>, label: &str, kind: Kind) -> Response {
    button_ex(ui, icon, label, kind, true)
}

pub fn button_ex(ui: &mut Ui, icon: Option<Icon>, label: &str, kind: Kind, enabled: bool) -> Response {
    let galley = ui.painter().layout_no_wrap(label.to_string(), font_button(), theme::INK);
    let icon_w = if icon.is_some() { 18.0 + 6.0 } else { 0.0 };
    let size = vec2(galley.size().x + icon_w + 28.0, theme::CONTROL_H);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    if ui.is_rect_visible(rect) {
        let hovered = enabled && resp.hovered();
        let pressed = enabled && resp.is_pointer_button_down_on();
        let (fill, stroke, fg) = match (kind, enabled) {
            (_, false) => (Color32::TRANSPARENT, theme::LINE, theme::DISABLED_INK),
            (Kind::Primary, true) => (if hovered { theme::ACCENT_HOVER } else { theme::ACCENT }, theme::ACCENT, theme::ON_ACCENT),
            (Kind::Outline, true) => (
                if hovered || pressed { theme::SURFACE_2 } else { Color32::TRANSPARENT },
                if hovered { theme::INK } else { theme::LINE_STRONG },
                theme::INK,
            ),
        };
        let p = ui.painter();
        p.rect(rect, CornerRadius::same(theme::PILL), fill, Stroke::new(1.0, stroke), StrokeKind::Inside);
        let mut x = rect.left() + 14.0;
        if let Some(ic) = icon {
            let r = egui::Rect::from_center_size(egui::pos2(x + 9.0, rect.center().y), vec2(18.0, 18.0));
            icons::paint(p, r, ic, fg);
            x += icon_w;
        }
        p.text(egui::pos2(x, rect.center().y), Align2::LEFT_CENTER, label, font_button(), fg);
        if resp.has_focus() {
            p.rect_stroke(rect.expand(3.0), CornerRadius::same(theme::PILL + 3), Stroke::new(2.0, theme::ACCENT), StrokeKind::Outside);
        }
    }
    resp
}

/// Round transparent icon button: muted, ink on a surface-2 fill when hovered.
pub fn icon_button(ui: &mut Ui, icon: Icon, label: &str, size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if ui.is_rect_visible(rect) {
        let hovered = resp.hovered();
        if hovered {
            ui.painter().circle_filled(rect.center(), size / 2.0, theme::SURFACE_2);
        }
        let ir = egui::Rect::from_center_size(rect.center(), Vec2::splat(size * 0.56));
        icons::paint(ui.painter(), ir, icon, if hovered { theme::INK } else { theme::MUTED });
    }
    resp.on_hover_text(label)
}

/// 30px pill toggle. Selected: check mark, accent-quiet fill, accent outline.
pub fn chip(ui: &mut Ui, selected: bool, label: &str) -> Response {
    chip_colored(ui, selected, label, None)
}

pub fn chip_colored(ui: &mut Ui, selected: bool, label: &str, dot: Option<Color32>) -> Response {
    let galley = ui.painter().layout_no_wrap(label.to_string(), font_button(), theme::INK);
    let lead = if selected { 16.0 } else { 0.0 } + if dot.is_some() { 12.0 } else { 0.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + lead + 24.0, 30.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let hovered = resp.hovered();
        let (fill, stroke) = if selected {
            (theme::ACCENT_QUIET, theme::ACCENT)
        } else if hovered {
            (theme::SURFACE_2, theme::INK)
        } else {
            (Color32::TRANSPARENT, theme::LINE_STRONG)
        };
        let p = ui.painter();
        p.rect(rect, CornerRadius::same(15), fill, Stroke::new(1.0, stroke), StrokeKind::Inside);
        let mut x = rect.left() + 12.0;
        if selected {
            let c = rect.center().y;
            p.line(
                vec![egui::pos2(x, c), egui::pos2(x + 3.5, c + 3.5), egui::pos2(x + 10.0, c - 3.5)],
                Stroke::new(1.6, theme::ACCENT),
            );
            x += 16.0;
        }
        if let Some(c) = dot {
            p.circle_filled(egui::pos2(x + 3.5, rect.center().y), 3.5, c);
            x += 12.0;
        }
        p.text(egui::pos2(x, rect.center().y), Align2::LEFT_CENTER, label, font_button(), theme::INK);
    }
    resp
}

/// 44x26 switch. Off: surface-2 track, border-control outline. On: accent track, dark thumb.
pub fn switch(ui: &mut Ui, on: &mut bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(44.0, 26.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool_with_time(resp.id, *on, 0.2);
        let p = ui.painter();
        let (track, outline) = if *on { (theme::ACCENT, theme::ACCENT) } else { (theme::SURFACE_2, theme::BORDER_CONTROL) };
        p.rect(rect, CornerRadius::same(13), track, Stroke::new(1.0, outline), StrokeKind::Inside);
        let x = egui::lerp(rect.left() + 13.0..=rect.right() - 13.0, t);
        let thumb = if *on { theme::ON_ACCENT } else { theme::INK };
        p.circle_filled(egui::pos2(x, rect.center().y), 9.0, thumb);
        if resp.hovered() {
            p.rect_stroke(rect.expand(2.0), CornerRadius::same(15), Stroke::new(1.0, theme::LINE_STRONG), StrokeKind::Outside);
        }
    }
    resp
}

/// Raised box on the background: surface fill, hairline edge, 14px radius.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(theme::SURFACE)
        .stroke(Stroke::new(1.0, theme::LINE))
        .corner_radius(CornerRadius::same(theme::RADIUS))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
            add(ui)
        })
        .inner
}

pub fn title(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).font(FontId::new(14.5, theme::semibold())));
}

pub fn caption(ui: &mut Ui, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).size(12.5).color(theme::MUTED));
}

/// `label        [-]  value  [+]`
pub fn stepper(ui: &mut Ui, label: &str, value: &str, minus: Icon, plus: Icon, mut step: impl FnMut(i32)) {
    ui.horizontal(|ui| {
        ui.add_sized([74.0, 28.0], egui::Label::new(RichText::new(label).color(theme::MUTED).size(13.0)));
        if round_step(ui, minus, "Decrease").clicked() {
            step(-1);
        }
        ui.add_sized([104.0, 28.0], egui::Label::new(RichText::new(value).font(FontId::new(14.0, theme::mono_bold()))));
        if round_step(ui, plus, "Increase").clicked() {
            step(1);
        }
    });
}

fn round_step(ui: &mut Ui, icon: Icon, label: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let hovered = resp.hovered();
        let p = ui.painter();
        p.circle(
            rect.center(),
            13.5,
            if hovered { theme::SURFACE_2 } else { Color32::TRANSPARENT },
            Stroke::new(1.0, if hovered { theme::INK } else { theme::LINE_STRONG }),
        );
        icons::paint(p, rect.shrink(6.0), icon, theme::INK);
    }
    resp.on_hover_text(label)
}

/// Small status pill: coloured dot and text on surface-2.
pub fn status_pill(ui: &mut Ui, dot: Color32, text: &str) {
    let font = FontId::new(13.0, egui::FontFamily::Proportional);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font.clone(), theme::INK);
    let (rect, _) = ui.allocate_exact_size(vec2(galley.size().x + 34.0, 28.0), Sense::hover());
    let p = ui.painter();
    p.rect(rect, CornerRadius::same(14), theme::SURFACE_2, Stroke::new(1.0, theme::LINE), StrokeKind::Inside);
    p.circle_filled(egui::pos2(rect.left() + 14.0, rect.center().y), 4.0, dot);
    p.text(egui::pos2(rect.left() + 24.0, rect.center().y), Align2::LEFT_CENTER, text, font, theme::INK);
}
