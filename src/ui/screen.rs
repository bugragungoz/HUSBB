//! The scope screen: graticule, traces, markers, cursors that read the real samples, hover readout.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Vec2, pos2, vec2,
};
use serde::{Deserialize, Serialize};

use husbb::acquire::Frame;
use husbb::export::time_axis;
use husbb::measure::{Measurements, si, si_opt};
use husbb::protocol::COUNTS_PER_DIV;

use super::theme;

pub const V_DIVS: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CursorMode {
    Off,
    /// Two vertical cursors snapped to samples; reads every visible channel at each.
    Track,
    /// Two horizontal cursors on one channel.
    Voltage,
}

#[derive(Debug, Clone)]
pub struct Cursors {
    pub mode: CursorMode,
    /// Horizontal positions in divisions from the trigger point.
    pub x: [f32; 2],
    /// Vertical positions in divisions from mid-screen.
    pub y: [f32; 2],
    pub channel: usize,
}

impl Default for Cursors {
    fn default() -> Self {
        Self { mode: CursorMode::Off, x: [-2.5, 2.5], y: [-1.0, 1.0], channel: 1 }
    }
}

/// A marker on one channel's sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Marker {
    pub channel: usize,
    pub index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LiveState {
    Offline,
    Waiting,
    Manual,
    Hold,
    Live(f32),
}

pub struct ScreenInput<'a> {
    pub frame: Option<&'a Frame>,
    pub view: [bool; 2],
    pub measurements: &'a [(usize, Measurements)],
    pub live: LiveState,
    pub overlay: bool,
}

#[derive(Default)]
pub struct ScreenState {
    pub cursors: Cursors,
    pub markers: Vec<Marker>,
}

// --- sample lookup, shared with the side panel --------------------------------------------------

fn dx_x0(f: &Frame) -> (f64, f64) {
    if f.settings.x_increment > 0.0 {
        (f.settings.x_increment, f.settings.x_origin)
    } else {
        let dx = 1.0 / f.acq.header.sample_rate;
        (dx, f.acq.points() as f64 / 2.0 * dx)
    }
}

/// Index of the sample nearest to a horizontal position in divisions.
pub fn index_at(f: &Frame, x_div: f32) -> Option<usize> {
    let n = f.acq.points();
    if n == 0 || f.settings.timebase <= 0.0 {
        return None;
    }
    let (dx, x0) = dx_x0(f);
    let i = ((x_div as f64 * f.settings.timebase + x0) / dx).round();
    Some(i.clamp(0.0, (n - 1) as f64) as usize)
}

pub fn time_of(f: &Frame, i: usize) -> f64 {
    let (dx, x0) = dx_x0(f);
    i as f64 * dx - x0
}

pub fn volts_at(f: &Frame, ch: usize, i: usize) -> Option<f64> {
    let t = f.acq.trace(ch)?;
    let s = *t.samples.get(i)? as f64;
    Some((s - f.acq.header.offsets[ch - 1] as f64) * f.acq.volts_per_count(ch))
}

fn x_div_of(f: &Frame, i: usize) -> f32 {
    (time_of(f, i) / f.settings.timebase.max(1e-15)) as f32
}

fn y_div_of(f: &Frame, ch: usize, i: usize) -> Option<f32> {
    let s = *f.acq.trace(ch)?.samples.get(i)? as f64;
    Some((s / COUNTS_PER_DIV) as f32)
}

/// Track cursor readout: (time A, time B), and per visible channel (V at A, V at B).
pub type TrackReadout = ([f64; 2], Vec<(usize, [f64; 2])>);

pub fn track_readout(f: &Frame, c: &Cursors, view: [bool; 2]) -> Option<TrackReadout> {
    let ia = index_at(f, c.x[0])?;
    let ib = index_at(f, c.x[1])?;
    let chans = (1..=2usize)
        .filter(|&ch| view[ch - 1])
        .filter_map(|ch| Some((ch, [volts_at(f, ch, ia)?, volts_at(f, ch, ib)?])))
        .collect();
    Some(([time_of(f, ia), time_of(f, ib)], chans))
}

// --- geometry -------------------------------------------------------------------------------------

struct Geom {
    rect: Rect,
    hdivs: f32,
}

impl Geom {
    fn k(&self) -> Vec2 {
        vec2(self.rect.width() / self.hdivs, self.rect.height() / V_DIVS)
    }
    fn to_screen(&self, x_div: f32, y_div: f32) -> Pos2 {
        let k = self.k();
        pos2(self.rect.center().x + x_div * k.x, self.rect.center().y - y_div * k.y)
    }
    fn to_div(&self, p: Pos2) -> (f32, f32) {
        let k = self.k();
        ((p.x - self.rect.center().x) / k.x, (self.rect.center().y - p.y) / k.y)
    }
}

fn mono(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Monospace)
}

/// A translucent rounded panel with lines of text.
fn text_panel(p: &egui::Painter, anchor: Pos2, align: Align2, lines: &[(String, Color32)]) -> Rect {
    let galleys: Vec<_> = lines.iter().map(|(t, c)| p.layout_no_wrap(t.clone(), mono(12.5), *c)).collect();
    let w = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max);
    let h: f32 = galleys.iter().map(|g| g.size().y).sum::<f32>() + 2.0 * galleys.len().saturating_sub(1) as f32;
    let rect = align.anchor_size(anchor, vec2(w + 20.0, h + 14.0));
    p.rect(
        rect,
        CornerRadius::same(theme::RADIUS_SM),
        Color32::from_rgba_premultiplied(15, 14, 13, 225),
        Stroke::new(1.0, theme::LINE_STRONG),
        StrokeKind::Inside,
    );
    let mut y = rect.top() + 7.0;
    for g in galleys {
        let gh = g.size().y;
        p.galley(pos2(rect.left() + 10.0, y), g, theme::INK);
        y += gh + 2.0;
    }
    rect
}

fn pill(p: &egui::Painter, anchor: Pos2, align: Align2, dot: Color32, text: &str) {
    let g = p.layout_no_wrap(text.to_string(), FontId::new(12.5, theme::medium()), theme::INK);
    let rect = align.anchor_size(anchor, vec2(g.size().x + 30.0, 24.0));
    p.rect(
        rect,
        CornerRadius::same(12),
        Color32::from_rgba_premultiplied(30, 28, 26, 230),
        Stroke::new(1.0, theme::LINE_STRONG),
        StrokeKind::Inside,
    );
    p.circle_filled(pos2(rect.left() + 12.0, rect.center().y), 3.5, dot);
    p.galley(pos2(rect.left() + 21.0, rect.center().y - g.size().y / 2.0), g, theme::INK);
}

// --- painting ----------------------------------------------------------------------------------------

/// Paints the screen into all of `ui`'s available space; returns the screen rectangle.
pub fn show(ui: &mut egui::Ui, input: &ScreenInput, st: &mut ScreenState) -> Rect {
    let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click());
    let outer = resp.rect;
    painter.rect(outer, CornerRadius::same(theme::RADIUS), theme::SCREEN_BG, Stroke::new(1.0, theme::LINE), StrokeKind::Inside);
    let rect = Rect::from_min_max(outer.min + vec2(34.0, 42.0), outer.max - vec2(34.0, 16.0));

    let frame = input.frame;
    let hdivs = frame
        .and_then(|f| {
            let tb = f.settings.timebase;
            let (dx, _) = dx_x0(f);
            (tb > 0.0).then(|| (f.acq.points() as f64 * dx / tb) as f32)
        })
        .filter(|h| (4.0..=40.0).contains(h))
        .unwrap_or(16.0);
    let g = Geom { rect, hdivs };
    graticule(&painter, &g);
    let clip = painter.with_clip_rect(rect.expand(1.0));
    if let Some(f) = frame {
        traces(&clip, &g, f, input.view);
        level_markers(&painter, &g, f, input.view);
    }
    status(&painter, outer, &g, input);
    match frame {
        Some(f) => {
            let dragging = cursors(ui, &painter, &g, f, input.view, &mut st.cursors);
            markers(&resp, &painter, &g, f, input.view, &mut st.markers);
            if input.overlay {
                overlay(&painter, &g, input.measurements);
            }
            if !dragging {
                hover(&painter, &g, f, input.view, resp.hover_pos());
            }
        }
        None => {
            painter.text(rect.center(), Align2::CENTER_CENTER, "Waiting for the DSO2D15...", FontId::new(15.0, theme::medium()), theme::MUTED);
        }
    }
    outer
}

fn graticule(p: &egui::Painter, g: &Geom) {
    let r = g.rect;
    let half = (g.hdivs / 2.0).floor() as i32;
    for i in -half..=half {
        if i != 0 {
            let x = g.to_screen(i as f32, 0.0).x;
            p.extend(Shape::dotted_line(&[pos2(x, r.top()), pos2(x, r.bottom())], theme::GRID, 5.0, 0.75));
        }
    }
    for j in -3..=3 {
        if j != 0 {
            let y = g.to_screen(0.0, j as f32).y;
            p.extend(Shape::dotted_line(&[pos2(r.left(), y), pos2(r.right(), y)], theme::GRID, 5.0, 0.75));
        }
    }
    let axis = Stroke::new(1.0, theme::GRID_AXIS);
    let c = r.center();
    p.line_segment([pos2(r.left(), c.y), pos2(r.right(), c.y)], axis);
    p.line_segment([pos2(c.x, r.top()), pos2(c.x, r.bottom())], axis);
    let k = g.k();
    let mut x = 0.2;
    while x <= g.hdivs / 2.0 + 1e-3 {
        for s in [-1.0, 1.0] {
            let px = c.x + s * x * k.x;
            p.line_segment([pos2(px, c.y - 3.0), pos2(px, c.y + 3.0)], axis);
        }
        x += 0.2;
    }
    let mut y = 0.2;
    while y <= V_DIVS / 2.0 + 1e-3 {
        for s in [-1.0, 1.0] {
            let py = c.y - s * y * k.y;
            p.line_segment([pos2(c.x - 3.0, py), pos2(c.x + 3.0, py)], axis);
        }
        y += 0.2;
    }
    p.rect_stroke(r, 0.0, Stroke::new(1.0, theme::GRID_AXIS), StrokeKind::Middle);
}

fn traces(p: &egui::Painter, g: &Geom, f: &Frame, view: [bool; 2]) {
    let tb = f.settings.timebase.max(1e-15);
    let t = time_axis(&f.acq, &f.settings);
    for trace in &f.acq.traces {
        let ch = trace.channel;
        if ch > 2 || !view[ch - 1] {
            continue;
        }
        let pts: Vec<Pos2> = trace
            .samples
            .iter()
            .zip(&t)
            .map(|(&s, &ti)| g.to_screen((ti / tb) as f32, (s as f64 / COUNTS_PER_DIV).clamp(-4.3, 4.3) as f32))
            .collect();
        p.line(pts, Stroke::new(1.4, theme::channel(ch)));
    }
}

fn triangle(p: &egui::Painter, tip: Pos2, dir: Vec2, size: f32, color: Color32) {
    let n = vec2(-dir.y, dir.x);
    let base = tip - dir * size;
    p.add(Shape::convex_polygon(vec![tip, base + n * size * 0.6, base - n * size * 0.6], color, Stroke::NONE));
}

fn level_markers(p: &egui::Painter, g: &Geom, f: &Frame, view: [bool; 2]) {
    let r = g.rect;
    for ch in 1..=2usize {
        if !view[ch - 1] || f.acq.trace(ch).is_none() {
            continue;
        }
        let y = (f.acq.header.offsets[ch - 1] as f64 / COUNTS_PER_DIV).clamp(-4.0, 4.0) as f32;
        let tip = pos2(r.left() - 3.0, g.to_screen(0.0, y).y);
        triangle(p, tip, vec2(1.0, 0.0), 12.0, theme::channel(ch));
        p.text(tip - vec2(17.0, 0.0), Align2::RIGHT_CENTER, ch.to_string(), FontId::new(12.0, theme::mono_bold()), theme::channel(ch));
    }
    let src = f.settings.trigger_source;
    if (1..=2).contains(&src) && f.acq.trace(src).is_some() {
        let vdiv = f.acq.header.volts_per_div[src - 1];
        if vdiv > 0.0 {
            let y = (f.acq.header.offsets[src - 1] as f64 / COUNTS_PER_DIV + f.settings.trigger_level / vdiv).clamp(-4.0, 4.0) as f32;
            let tip = pos2(r.right() + 3.0, g.to_screen(0.0, y).y);
            triangle(p, tip, vec2(-1.0, 0.0), 12.0, theme::TRIGGER);
            p.text(tip + vec2(15.0, 0.0), Align2::LEFT_CENTER, "T", FontId::new(12.0, theme::mono_bold()), theme::TRIGGER);
        }
    }
    triangle(p, pos2(r.center().x, r.top() - 3.0), vec2(0.0, 1.0), 11.0, theme::TRIGGER);
}

fn status(p: &egui::Painter, outer: Rect, g: &Geom, input: &ScreenInput) {
    let y = outer.top() + 21.0;
    if let Some(f) = input.frame {
        let h = &f.acq.header;
        let (dot, text) = if h.running { (theme::OK, "Run") } else { (theme::WARN, "Stop") };
        let text = format!("{text}  -  {}", if h.triggered { "Triggered" } else { "Auto" });
        pill(p, pos2(g.rect.left(), y), Align2::LEFT_CENTER, dot, &text);
    }
    let (dot, text) = match input.live {
        LiveState::Offline => (theme::WARN, "Offline".to_string()),
        LiveState::Waiting => (theme::MUTED, "Waiting".to_string()),
        LiveState::Manual => (theme::INFO, "Link off".to_string()),
        LiveState::Hold => (theme::INFO, "Hold".to_string()),
        LiveState::Live(age) => {
            let fresh = (1.0 - age).clamp(0.0, 1.0);
            let c = if age < 3.0 { theme::ACCENT.gamma_multiply(0.55 + 0.45 * fresh) } else { theme::WARN };
            (c, format!("Live  {age:.1} s"))
        }
    };
    pill(p, pos2(g.rect.right(), y), Align2::RIGHT_CENTER, dot, &text);
}

fn overlay(p: &egui::Painter, g: &Geom, meas: &[(usize, Measurements)]) {
    if meas.is_empty() {
        return;
    }
    let lines: Vec<(String, Color32)> = meas
        .iter()
        .map(|(ch, m)| {
            let mut s = format!(
                "CH{ch}  f {}   Vpp {}   Vrms {}   Vavg {}",
                si_opt(m.freq, "Hz"),
                si(m.vpp, "V"),
                si(m.vrms, "V"),
                si(m.vavg, "V")
            );
            if let Some(d) = m.duty {
                s += &format!("   duty {d:.1}%");
            }
            (s, theme::channel(*ch))
        })
        .collect();
    text_panel(p, g.rect.left_bottom() + vec2(8.0, -8.0), Align2::LEFT_BOTTOM, &lines);
}

/// Returns true while a cursor is being dragged.
fn cursors(ui: &mut egui::Ui, p: &egui::Painter, g: &Geom, f: &Frame, view: [bool; 2], c: &mut Cursors) -> bool {
    let r = g.rect;
    let k = g.k();
    let stroke = Stroke::new(1.2, theme::CURSOR);
    let mut dragging = false;
    match c.mode {
        CursorMode::Off => {}
        CursorMode::Track => {
            for i in 0..2 {
                // snap to the sample under the cursor so every reading is a real sample
                let idx = index_at(f, c.x[i]).unwrap_or(0);
                let x_div = x_div_of(f, idx);
                let x = g.to_screen(x_div, 0.0).x;
                let band = Rect::from_min_max(pos2(x - 6.0, r.top()), pos2(x + 6.0, r.bottom()));
                let resp = ui.interact(band, egui::Id::unique(("husbb_track", i)), Sense::drag());
                if resp.dragged() {
                    dragging = true;
                    c.x[i] = (c.x[i] + resp.drag_delta().x / k.x).clamp(-g.hdivs / 2.0, g.hdivs / 2.0);
                } else {
                    c.x[i] = x_div;
                }
                if resp.hovered() || resp.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                }
                p.extend(Shape::dashed_line(&[pos2(x, r.top()), pos2(x, r.bottom())], stroke, 6.0, 4.0));
                p.text(pos2(x + 4.0, r.top() + 4.0), Align2::LEFT_TOP, ["A", "B"][i], FontId::new(12.0, theme::mono_bold()), theme::CURSOR);
                for ch in 1..=2usize {
                    if view[ch - 1]
                        && let Some(y) = y_div_of(f, ch, idx)
                    {
                        let pt = g.to_screen(x_div, y.clamp(-4.0, 4.0));
                        p.circle(pt, 4.0, theme::channel(ch), Stroke::new(1.5, theme::SCREEN_BG));
                    }
                }
            }
            if let Some(([ta, tb], chans)) = track_readout(f, c, view) {
                let dt = (tb - ta).abs();
                let mut lines = vec![(
                    format!(
                        "A {}   B {}   dt {}   1/dt {}",
                        si(ta, "s"),
                        si(tb, "s"),
                        si(dt, "s"),
                        if dt > 0.0 { si(1.0 / dt, "Hz") } else { "--".into() }
                    ),
                    theme::CURSOR,
                )];
                for (ch, [va, vb]) in chans {
                    lines.push((format!("CH{ch}  A {}   B {}   dV {}", si(va, "V"), si(vb, "V"), si(vb - va, "V")), theme::channel(ch)));
                }
                text_panel(p, r.right_top() + vec2(-8.0, 8.0), Align2::RIGHT_TOP, &lines);
            }
        }
        CursorMode::Voltage => {
            let ch = c.channel;
            for i in 0..2 {
                let y = g.to_screen(0.0, c.y[i]).y;
                let band = Rect::from_min_max(pos2(r.left(), y - 6.0), pos2(r.right(), y + 6.0));
                let resp = ui.interact(band, egui::Id::unique(("husbb_volt", i)), Sense::drag());
                if resp.dragged() {
                    dragging = true;
                    c.y[i] = (c.y[i] - resp.drag_delta().y / k.y).clamp(-V_DIVS / 2.0, V_DIVS / 2.0);
                }
                if resp.hovered() || resp.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                }
                p.extend(Shape::dashed_line(&[pos2(r.left(), y), pos2(r.right(), y)], stroke, 6.0, 4.0));
                p.text(pos2(r.left() + 4.0, y - 2.0), Align2::LEFT_BOTTOM, ["A", "B"][i], FontId::new(12.0, theme::mono_bold()), theme::CURSOR);
            }
            let vdiv = f.acq.header.volts_per_div[ch - 1];
            let off = f.acq.header.offsets[ch - 1] as f64 / COUNTS_PER_DIV;
            let v = |y: f32| (y as f64 - off) * vdiv;
            let lines = vec![(
                format!("CH{ch}  A {}   B {}   dV {}", si(v(c.y[0]), "V"), si(v(c.y[1]), "V"), si(v(c.y[1]) - v(c.y[0]), "V")),
                theme::channel(ch),
            )];
            text_panel(p, r.right_top() + vec2(-8.0, 8.0), Align2::RIGHT_TOP, &lines);
        }
    }
    dragging
}

/// Double-click drops a marker on the nearest visible trace; right-click removes the nearest one.
fn markers(resp: &egui::Response, p: &egui::Painter, g: &Geom, f: &Frame, view: [bool; 2], list: &mut Vec<Marker>) {
    let pos_of = |m: &Marker| -> Option<Pos2> {
        let y = y_div_of(f, m.channel, m.index)?;
        Some(g.to_screen(x_div_of(f, m.index), y.clamp(-4.0, 4.0)))
    };
    if let Some(pt) = resp.interact_pointer_pos() {
        if resp.double_clicked() && g.rect.contains(pt) {
            let (x_div, _) = g.to_div(pt);
            if let Some(idx) = index_at(f, x_div) {
                let best = (1..=2usize)
                    .filter(|&ch| view[ch - 1])
                    .filter_map(|ch| Some((ch, (g.to_screen(0.0, y_div_of(f, ch, idx)?).y - pt.y).abs())))
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                if let Some((ch, _)) = best {
                    list.push(Marker { channel: ch, index: idx });
                }
            }
        }
        if resp.secondary_clicked()
            && let Some((i, _)) = list
                .iter()
                .enumerate()
                .filter_map(|(i, m)| Some((i, pos_of(m)?.distance(pt))))
                .filter(|(_, d)| *d < 14.0)
                .min_by(|a, b| a.1.total_cmp(&b.1))
        {
            list.remove(i);
        }
    }
    for (n, m) in list.iter().enumerate() {
        if !view[m.channel - 1] {
            continue;
        }
        let Some(pt) = pos_of(m) else { continue };
        p.circle(pt, 5.0, theme::channel(m.channel), Stroke::new(1.5, theme::INK));
        let value = volts_at(f, m.channel, m.index).map(|v| si(v, "V")).unwrap_or_default();
        p.text(
            pt + vec2(8.0, -8.0),
            Align2::LEFT_BOTTOM,
            format!("M{}  {}  {value}", n + 1, si(time_of(f, m.index), "s")),
            mono(12.0),
            theme::INK,
        );
    }
}

fn hover(p: &egui::Painter, g: &Geom, f: &Frame, view: [bool; 2], at: Option<Pos2>) {
    let Some(pos) = at.filter(|pt| g.rect.contains(*pt)) else { return };
    let (x_div, _) = g.to_div(pos);
    let Some(i) = index_at(f, x_div) else { return };
    let mut lines = vec![(format!("t {}", si(time_of(f, i), "s")), theme::INK)];
    for ch in 1..=2usize {
        if view[ch - 1]
            && let Some(v) = volts_at(f, ch, i)
        {
            lines.push((format!("CH{ch} {}", si(v, "V")), theme::channel(ch)));
        }
    }
    let x = g.to_screen(x_div_of(f, i), 0.0).x;
    p.line_segment([pos2(x, g.rect.top()), pos2(x, g.rect.bottom())], Stroke::new(1.0, theme::LINE_STRONG));
    let right_side = pos.x + 160.0 < g.rect.right();
    let anchor = if right_side { pos + vec2(14.0, 14.0) } else { pos + vec2(-14.0, 14.0) };
    text_panel(p, anchor, if right_side { Align2::LEFT_TOP } else { Align2::RIGHT_TOP }, &lines);
}
