//! Single-stroke outline icons on a 24-unit grid (Bugra: 1.6 stroke, round caps and joins),
//! drawn with the painter so they follow the theme colours.

use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2};
use eframe::egui::epaint::{CircleShape, PathShape, PathStroke};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Play,
    Stop,
    Single,
    Auto,
    Export,
    SaveAs,
    Settings,
    Close,
    Minus,
    Plus,
    Up,
    Down,
}

/// Paints `icon` into `rect` (square), in `color`.
pub fn paint(p: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let s = rect.width().min(rect.height()) / 24.0;
    let o = rect.center() - eframe::egui::vec2(12.0 * s, 12.0 * s);
    let at = |x: f32, y: f32| -> Pos2 { pos2(o.x + x * s, o.y + y * s) };
    let stroke = Stroke::new((1.6 * s).max(1.2), color);
    let line = |pts: &[(f32, f32)]| {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| at(x, y)).collect();
        p.add(Shape::Path(PathShape::line(v, PathStroke::new(stroke.width, color))));
    };
    let closed = |pts: &[(f32, f32)]| {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| at(x, y)).collect();
        p.add(Shape::Path(PathShape::closed_line(v, PathStroke::new(stroke.width, color))));
    };
    let circle = |x: f32, y: f32, r: f32| {
        p.add(Shape::Circle(CircleShape::stroke(at(x, y), r * s, stroke)));
    };
    match icon {
        Icon::Play => closed(&[(8.0, 5.5), (18.5, 12.0), (8.0, 18.5)]),
        Icon::Stop => closed(&[(6.5, 6.5), (17.5, 6.5), (17.5, 17.5), (6.5, 17.5)]),
        Icon::Single => {
            // one pulse: baseline, rise, top, fall, baseline
            line(&[(3.5, 17.0), (8.0, 17.0), (8.0, 7.0), (16.0, 7.0), (16.0, 17.0), (20.5, 17.0)]);
        }
        Icon::Auto => {
            line(&[(5.0, 19.0), (15.0, 9.0)]);
            line(&[(17.5, 3.5), (17.5, 7.5)]);
            line(&[(15.5, 5.5), (19.5, 5.5)]);
            line(&[(8.0, 4.5), (8.0, 7.5)]);
            line(&[(6.5, 6.0), (9.5, 6.0)]);
        }
        Icon::Export => {
            line(&[(12.0, 4.0), (12.0, 14.5)]);
            line(&[(7.5, 10.0), (12.0, 14.5), (16.5, 10.0)]);
            line(&[(4.5, 15.5), (4.5, 19.5), (19.5, 19.5), (19.5, 15.5)]);
        }
        Icon::SaveAs => {
            closed(&[(3.5, 7.0), (9.5, 7.0), (11.5, 9.0), (20.5, 9.0), (20.5, 18.5), (3.5, 18.5)]);
        }
        Icon::Settings => {
            // three sliders
            for (y, k) in [(7.0, 15.0), (12.0, 8.5), (17.0, 13.0)] {
                line(&[(4.0, y), (k - 2.2, y)]);
                line(&[(k + 2.2, y), (20.0, y)]);
                circle(k, y, 2.2);
            }
        }
        Icon::Close => {
            line(&[(6.5, 6.5), (17.5, 17.5)]);
            line(&[(17.5, 6.5), (6.5, 17.5)]);
        }
        Icon::Minus => line(&[(6.0, 12.0), (18.0, 12.0)]),
        Icon::Plus => {
            line(&[(6.0, 12.0), (18.0, 12.0)]);
            line(&[(12.0, 6.0), (12.0, 18.0)]);
        }
        Icon::Up => line(&[(6.5, 14.5), (12.0, 9.0), (17.5, 14.5)]),
        Icon::Down => line(&[(6.5, 9.5), (12.0, 15.0), (17.5, 9.5)]),
    }
}

/// The HUSBB mark: warm-black rounded tile, hairline edge, green sine (same as assets/icon.svg).
pub fn paint_logo(p: &Painter, rect: Rect) {
    use super::theme;
    let r = (rect.width() * 0.22) as u8;
    p.rect(rect, r, theme::BG, Stroke::new(1.0, theme::LINE_STRONG), eframe::egui::StrokeKind::Inside);
    let w = rect.width();
    let pts: Vec<Pos2> = (0..=48)
        .map(|i| {
            let t = i as f32 / 48.0;
            pos2(rect.left() + w * (0.2 + 0.6 * t), rect.center().y - w * 0.19 * (t * std::f32::consts::TAU).sin())
        })
        .collect();
    p.add(Shape::Path(PathShape::line(pts, PathStroke::new((w * 0.075).max(1.5), theme::ACCENT))));
}
