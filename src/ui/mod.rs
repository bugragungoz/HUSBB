//! HUSBB window: top bar, scope screen, side panel (Scope / Measure), status bar, settings.

mod icons;
mod screen;
pub mod theme;
mod widgets;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::{self, Align, Color32, CornerRadius, FontId, Layout, Margin, RichText, Stroke, Ui, vec2};
use serde::{Deserialize, Serialize};

use husbb::acquire::{Acquirer, Command, Frame, Shared, Status};
use husbb::export::write_csv;
use husbb::measure::{Measurements, measure, si, si_opt, step_125};
use husbb::protocol::COUNTS_PER_DIV;
use husbb::scope::{Slope, Sweep};

use icons::Icon;
use screen::{CursorMode, LiveState, ScreenInput, ScreenState};
use widgets::Kind;

pub const APP_NAME: &str = "HUSBB";
pub const MODEL: &str = "DSO2D15";

/// Remembered between runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub export_dir: Option<PathBuf>,
    pub export_png: bool,
    pub export_csv: bool,
    pub overlay: bool,
    pub view: [bool; 2],
}

impl Default for Prefs {
    fn default() -> Self {
        Self { export_dir: None, export_png: true, export_csv: true, overlay: true, view: [true, true] }
    }
}

pub struct Options {
    pub resource: Option<String>,
    /// Save a screenshot of the window to this file after `screenshot_after`, then quit.
    pub screenshot: Option<PathBuf>,
    pub screenshot_after: Duration,
    pub cursors: Option<CursorMode>,
    pub tab: Tab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Scope,
    Measure,
}

pub struct HusbbApp {
    acq: Acquirer,
    prefs: Prefs,
    tab: Tab,
    screen: ScreenState,
    hold: Option<Frame>,
    screen_rect: egui::Rect,
    settings_open: bool,
    snack: Option<(String, Instant)>,
    auto_shot: Option<(PathBuf, Instant)>,
}

impl HusbbApp {
    pub fn new(cc: &eframe::CreationContext<'_>, opts: Options) -> Self {
        theme::install(&cc.egui_ctx);
        let prefs: Prefs = cc.storage.and_then(|s| eframe::get_value(s, eframe::APP_KEY)).unwrap_or_default();
        let ctx = cc.egui_ctx.clone();
        let acq = Acquirer::spawn(opts.resource, move || ctx.request_repaint());
        let mut screen = ScreenState::default();
        if let Some(mode) = opts.cursors {
            screen.cursors.mode = mode;
        }
        Self {
            acq,
            prefs,
            tab: opts.tab,
            screen,
            hold: None,
            screen_rect: egui::Rect::NOTHING,
            settings_open: false,
            snack: None,
            auto_shot: opts.screenshot.map(|p| (p, Instant::now() + opts.screenshot_after)),
        }
    }

    fn snack(&mut self, msg: impl Into<String>) {
        self.snack = Some((msg.into(), Instant::now()));
    }

    fn save_png(&self, ctx: &egui::Context, path: PathBuf, crop: Option<egui::Rect>, then_quit: bool) {
        let ppp = ctx.pixels_per_point();
        let quit_ctx = ctx.clone();
        ctx.request_screenshot(move |img| {
            match save_color_image(&img, crop, ppp, &path) {
                Ok(()) => log::info!("saved {}", path.display()),
                Err(e) => log::error!("saving {}: {e}", path.display()),
            }
            if then_quit {
                quit_ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }

    fn export_now(&mut self, ctx: &egui::Context, frame: Option<&Frame>) {
        let Some(dir) = self.prefs.export_dir.clone() else {
            self.settings_open = true;
            self.snack("Choose an export folder first.");
            return;
        };
        let Some(f) = frame else {
            self.snack("Error: there is no record to export yet.");
            return;
        };
        if !self.prefs.export_png && !self.prefs.export_csv {
            self.settings_open = true;
            self.snack("Turn on PNG or CSV export in Settings.");
            return;
        }
        let base = dir.join(format!("{APP_NAME}_{MODEL}_{}", chrono::Local::now().format("%Y%m%d_%H%M%S")));
        let mut saved = Vec::new();
        if self.prefs.export_csv {
            let chans: Vec<usize> = (1..=2).filter(|&c| self.prefs.view[c - 1]).collect();
            match write_csv(&base.with_extension("csv"), &f.acq, &f.settings, &chans) {
                Ok(()) => saved.push("CSV"),
                Err(e) => {
                    self.snack(format!("Error: CSV export failed: {e}"));
                    return;
                }
            }
        }
        if self.prefs.export_png {
            self.save_png(ctx, base.with_extension("png"), Some(self.screen_rect), false);
            saved.push("PNG");
        }
        self.snack(format!("Exported {} to {}", saved.join(" and "), dir.display()));
    }
}

fn save_color_image(img: &egui::ColorImage, crop: Option<egui::Rect>, ppp: f32, path: &Path) -> Result<(), String> {
    let [w, h] = img.size;
    let (x0, y0, x1, y1) = match crop {
        Some(r) => (
            ((r.min.x * ppp) as usize).min(w),
            ((r.min.y * ppp) as usize).min(h),
            ((r.max.x * ppp).ceil() as usize).min(w),
            ((r.max.y * ppp).ceil() as usize).min(h),
        ),
        None => (0, 0, w, h),
    };
    let mut out = image::RgbaImage::new((x1 - x0) as u32, (y1 - y0) as u32);
    for y in y0..y1 {
        for x in x0..x1 {
            let c = img.pixels[y * w + x];
            out.put_pixel((x - x0) as u32, (y - y0) as u32, image::Rgba([c.r(), c.g(), c.b(), 255]));
        }
    }
    out.save(path).map_err(|e| e.to_string())
}

fn channel_measurements(frame: &Frame, view: [bool; 2]) -> Vec<(usize, Measurements)> {
    (1..=2usize)
        .filter(|&ch| view[ch - 1])
        .filter_map(|ch| {
            let v = frame.acq.volts(ch)?;
            Some((ch, measure(&v, frame.acq.header.sample_rate, frame.acq.volts_per_count(ch))?))
        })
        .collect()
}

fn panel_frame(margin: Margin) -> egui::Frame {
    egui::Frame::new().fill(theme::BG).inner_margin(margin)
}

impl eframe::App for HusbbApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let shared = self.acq.shared();
        let frame = self.hold.clone().or_else(|| shared.frame.clone());
        let measurements = frame.as_ref().map(|f| channel_measurements(f, self.prefs.view)).unwrap_or_default();

        egui::Panel::top("top").exact_size(62.0).frame(panel_frame(Margin::symmetric(18, 0))).show(ui, |ui| {
            self.top_bar(ui, &ctx, &shared, frame.as_ref());
        });
        egui::Panel::bottom("status").exact_size(30.0).frame(panel_frame(Margin::symmetric(18, 0))).show(ui, |ui| {
            status_bar(ui, &shared, frame.as_ref());
        });
        egui::Panel::right("side")
            .exact_size(336.0)
            .resizable(false)
            .frame(panel_frame(Margin { left: 4, right: 18, top: 14, bottom: 14 }))
            .show(ui, |ui| {
                self.side_panel(ui, &shared, frame.as_ref(), &measurements);
            });
        egui::CentralPanel::default().frame(panel_frame(Margin { left: 18, right: 14, top: 14, bottom: 14 })).show(ui, |ui| {
            let live = if self.hold.is_some() {
                LiveState::Hold
            } else {
                match (&shared.status, frame.as_ref()) {
                    (Status::Manual { .. }, _) => LiveState::Manual,
                    (Status::Connected { .. }, Some(f)) => LiveState::Live(f.changed_at.elapsed().as_secs_f32()),
                    (Status::Connected { .. }, None) | (Status::Starting | Status::Searching | Status::Connecting, _) => LiveState::Waiting,
                    _ => LiveState::Offline,
                }
            };
            let input = ScreenInput { frame: frame.as_ref(), view: self.prefs.view, measurements: &measurements, live, overlay: self.prefs.overlay };
            self.screen_rect = screen::show(ui, &input, &mut self.screen);
            self.paint_snack(ui);
        });

        self.settings_dialog(&ctx);

        if let Some((path, at)) = self.auto_shot.clone()
            && Instant::now() >= at
        {
            self.auto_shot = None;
            self.save_png(&ctx, path, None, true);
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.prefs);
    }
}

// --- top bar ------------------------------------------------------------------------------------

impl HusbbApp {
    fn top_bar(&mut self, ui: &mut Ui, ctx: &egui::Context, shared: &Shared, frame: Option<&Frame>) {
        ui.horizontal_centered(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(30.0, 30.0), egui::Sense::hover());
            icons::paint_logo(ui.painter(), r);
            ui.add_space(4.0);
            ui.label(RichText::new(APP_NAME).font(FontId::new(17.0, theme::semibold())));
            ui.label(RichText::new(format!("Hantek {MODEL} USB Bridge")).color(theme::MUTED).size(13.5));
            ui.add_space(10.0);
            let (dot, text) = status_text(&shared.status);
            widgets::status_pill(ui, dot, &text);

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::icon_button(ui, Icon::Settings, "Settings", 34.0).clicked() {
                    self.settings_open = true;
                }
                let save_as = widgets::button(ui, Some(Icon::SaveAs), "Save as...", Kind::Outline);
                egui::Popup::menu(&save_as).show(|ui| {
                    ui.set_min_width(170.0);
                    if ui.button("Screen as PNG...").clicked() {
                        if let Some(path) = rfd::FileDialog::new().set_file_name(file_name("png")).add_filter("PNG", &["png"]).save_file() {
                            self.save_png(ctx, path.clone(), Some(self.screen_rect), false);
                            self.snack(format!("Saved {}", path.display()));
                        }
                        ui.close();
                    }
                    if ui.button("Data as CSV...").clicked() {
                        if let Some(f) = frame
                            && let Some(path) = rfd::FileDialog::new().set_file_name(file_name("csv")).add_filter("CSV", &["csv"]).save_file()
                        {
                            let chans: Vec<usize> = (1..=2).filter(|&c| self.prefs.view[c - 1]).collect();
                            match write_csv(&path, &f.acq, &f.settings, &chans) {
                                Ok(()) => self.snack(format!("Saved {}", path.display())),
                                Err(e) => self.snack(format!("Error: {e}")),
                            }
                        }
                        ui.close();
                    }
                });
                let export = widgets::button(ui, Some(Icon::Export), "Export", Kind::Primary).on_hover_text(match &self.prefs.export_dir {
                    Some(d) => format!("Save the current view to {}", d.display()),
                    None => "Choose an export folder in Settings first".to_string(),
                });
                if export.clicked() {
                    self.export_now(ctx, frame);
                }
                ui.add_space(6.0);
                separator(ui);

                let holding = self.hold.is_some();
                if widgets::chip(ui, holding, "Hold").on_hover_text("Freeze the screen; the scope keeps running").clicked() {
                    self.hold = if holding { None } else { shared.frame.clone() };
                }
                separator(ui);

                let linked = self.acq.link();
                let scope_ready = linked && matches!(shared.status, Status::Connected { .. });
                if widgets::button_ex(ui, Some(Icon::Auto), "Auto setup", Kind::Outline, scope_ready)
                    .on_hover_text("Auto setup on the scope (changes every setting)")
                    .clicked()
                {
                    self.acq.send(Command::AutoSetup);
                }
                if widgets::button_ex(ui, Some(Icon::Single), "Single", Kind::Outline, scope_ready).clicked() {
                    self.acq.send(Command::Single);
                }
                let running = frame.is_none_or(|f| f.acq.header.running);
                let (icon, label) = if running { (Icon::Stop, "Stop") } else { (Icon::Play, "Run") };
                if widgets::button_ex(ui, Some(icon), label, Kind::Outline, scope_ready).clicked() {
                    self.acq.send(if running { Command::Stop } else { Command::Run });
                }
                separator(ui);

                let mut link = linked;
                let resp = widgets::switch(ui, &mut link).on_hover_text(
                    "On: read the DSO2D15 continuously (its front panel is locked).\nOff: send nothing; the front panel unlocks after about 5 s.",
                );
                if resp.changed() {
                    self.acq.set_link(link);
                    self.snack(if link { "Live link on" } else { "Live link off - the DSO2D15 front panel unlocks in about 5 s" });
                }
                ui.label(RichText::new("Live link").color(theme::MUTED).size(13.5));
            });
        });
    }

    fn paint_snack(&mut self, ui: &mut Ui) {
        let Some((msg, at)) = &self.snack else { return };
        let age = at.elapsed().as_secs_f32();
        if age > 5.0 {
            self.snack = None;
            return;
        }
        let alpha = (5.0 - age).clamp(0.0, 0.3) / 0.3;
        let p = ui.painter();
        let font = FontId::new(13.5, egui::FontFamily::Proportional);
        let g = p.layout_no_wrap(msg.clone(), font, theme::INK.gamma_multiply(alpha));
        let r = egui::Rect::from_center_size(self.screen_rect.center_bottom() - vec2(0.0, 70.0), g.size() + vec2(32.0, 20.0));
        p.rect(
            r,
            CornerRadius::same(theme::RADIUS_SM),
            theme::SURFACE_2.gamma_multiply(alpha),
            Stroke::new(1.0, theme::LINE_STRONG.gamma_multiply(alpha)),
            egui::StrokeKind::Inside,
        );
        p.galley(r.min + vec2(16.0, 10.0), g, theme::INK);
    }
}

fn separator(ui: &mut Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(1.0, 26.0), egui::Sense::hover());
    ui.painter().line_segment([r.center_top(), r.center_bottom()], Stroke::new(1.0, theme::LINE_STRONG));
}

fn file_name(ext: &str) -> String {
    format!("{APP_NAME}_{MODEL}_{}.{ext}", chrono::Local::now().format("%Y%m%d_%H%M%S"))
}

fn status_text(status: &Status) -> (Color32, String) {
    match status {
        Status::Starting => (theme::MUTED, "Starting...".into()),
        Status::NoVisa(e) => (theme::WARN, format!("Error: no VISA library ({e})")),
        Status::Searching => (theme::MUTED, format!("Searching for the {MODEL}...")),
        Status::Connecting => (theme::MUTED, format!("Connecting to the {MODEL}...")),
        Status::Connected { idn, .. } => {
            let fw = idn.split(',').nth(3).unwrap_or("").trim().to_string();
            (theme::OK, format!("Connected  -  {MODEL}  fw {fw}"))
        }
        Status::Lost(_) => (theme::WARN, "Error: disconnected - re-plug the USB cable".into()),
        Status::Manual { .. } => match status.panel_unlock_in() {
            Some(left) if !left.is_zero() => (theme::INFO, format!("Link off  -  front panel free in {} s", left.as_secs() + 1)),
            _ => (theme::INFO, "Link off  -  front panel free".into()),
        },
    }
}

// --- side panel -------------------------------------------------------------------------------------

impl HusbbApp {
    fn side_panel(&mut self, ui: &mut Ui, shared: &Shared, frame: Option<&Frame>, meas: &[(usize, Measurements)]) {
        // everything fits at the default window size; the scroll bar only appears in small windows
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| self.side_content(ui, shared, frame, meas));
    }

    fn side_content(&mut self, ui: &mut Ui, shared: &Shared, frame: Option<&Frame>, meas: &[(usize, Measurements)]) {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        ui.horizontal(|ui| {
            if widgets::chip(ui, self.tab == Tab::Scope, "Scope").clicked() {
                self.tab = Tab::Scope;
            }
            if widgets::chip(ui, self.tab == Tab::Measure, "Measure").clicked() {
                self.tab = Tab::Measure;
            }
        });
        let Some(f) = frame else {
            widgets::card(ui, |ui| {
                widgets::title(ui, "No data yet");
                widgets::caption(
                    ui,
                    match &shared.status {
                        Status::Manual { .. } => "The live link is off. Turn it on to read the DSO2D15.".to_string(),
                        _ => format!("Connect the {MODEL} to its rear USB port. On a laptop with only USB 3 ports, use a USB 2.0 hub."),
                    },
                );
            });
            return;
        };
        let editable = self.acq.link() && self.hold.is_none() && matches!(shared.status, Status::Connected { .. });
        match self.tab {
            Tab::Scope => {
                ui.add_enabled_ui(editable, |ui| self.scope_tab(ui, f));
                if !editable {
                    widgets::caption(ui, "Controls are off while the live link is off or the screen is on hold.");
                }
            }
            Tab::Measure => self.measure_tab(ui, f, meas),
        }
    }

    fn scope_tab(&mut self, ui: &mut Ui, f: &Frame) {
        let h = &f.acq.header;
        let s = &f.settings;
        for ch in 1..=2usize {
            widgets::card(ui, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(10.0, 20.0), egui::Sense::hover());
                    ui.painter().circle_filled(r.center(), 5.0, theme::channel(ch));
                    widgets::title(ui, &format!("CH{ch}"));
                    let view = self.prefs.view[ch - 1];
                    if widgets::chip(ui, view, "View").on_hover_text("Show or hide this trace in HUSBB only").clicked() {
                        self.prefs.view[ch - 1] = !view;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let mut on = h.enabled[ch - 1];
                        if widgets::switch(ui, &mut on).on_hover_text("Channel on or off at the DSO2D15").changed() {
                            self.acq.send(Command::ChannelDisplay(ch, on));
                        }
                        ui.label(RichText::new("On scope").color(theme::MUTED).size(13.0));
                    });
                });
                let vdiv = h.volts_per_div[ch - 1];
                widgets::stepper(ui, "V/div", &si(vdiv, "V"), Icon::Minus, Icon::Plus, |d| {
                    self.acq.send(Command::ChannelScale(ch, step_125(vdiv, d)));
                });
                let off_div = h.offsets[ch - 1] as f64 / COUNTS_PER_DIV;
                widgets::stepper(ui, "Position", &format!("{off_div:+.1} div"), Icon::Down, Icon::Up, |d| {
                    self.acq.send(Command::ChannelOffset(ch, (off_div + 0.2 * d as f64) * vdiv));
                });
                widgets::caption(ui, format!("{} coupling  -  {} probe", coupling(s, ch), probe(s, ch)));
            });
        }
        widgets::card(ui, |ui| {
            widgets::title(ui, "Horizontal");
            let tb = s.timebase;
            widgets::stepper(ui, "Time/div", &si(tb, "s"), Icon::Minus, Icon::Plus, |d| self.acq.send(Command::Timebase(step_125(tb, d))));
            widgets::caption(ui, format!("{}  -  {} points", si(h.sample_rate, "Sa/s"), f.acq.points()));
        });
        widgets::card(ui, |ui| {
            widgets::title(ui, "Trigger");
            ui.horizontal(|ui| {
                for c in 1..=2 {
                    if widgets::chip_colored(ui, s.trigger_source == c, &format!("CH{c}"), Some(theme::channel(c))).clicked() {
                        self.acq.send(Command::TriggerSource(c));
                    }
                }
            });
            ui.horizontal(|ui| {
                for (sl, name) in [(Slope::Rising, "Rising"), (Slope::Falling, "Falling"), (Slope::Either, "Either")] {
                    if widgets::chip(ui, s.trigger_slope == sl, name).clicked() {
                        self.acq.send(Command::TriggerSlope(sl));
                    }
                }
            });
            ui.horizontal(|ui| {
                for sw in Sweep::ALL {
                    if widgets::chip(ui, s.trigger_sweep == sw, sw.label()).clicked() {
                        self.acq.send(Command::TriggerSweep(sw));
                    }
                }
            });
            let src = s.trigger_source.clamp(1, 2);
            let step = 0.2 * h.volts_per_div[src - 1];
            let level = s.trigger_level;
            widgets::stepper(ui, "Level", &si(level, "V"), Icon::Down, Icon::Up, |d| self.acq.send(Command::TriggerLevel(level + step * d as f64)));
        });
    }

    fn measure_tab(&mut self, ui: &mut Ui, f: &Frame, meas: &[(usize, Measurements)]) {
        widgets::card(ui, |ui| {
            ui.horizontal(|ui| {
                widgets::title(ui, "Measurements");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    widgets::switch(ui, &mut self.prefs.overlay).on_hover_text("Also write them on the screen");
                    ui.label(RichText::new("On screen").color(theme::MUTED).size(13.0));
                });
            });
            ui.spacing_mut().interact_size.y = 18.0;
            egui::Grid::new("meas").num_columns(1 + meas.len()).spacing([18.0, 3.0]).min_col_width(70.0).striped(true).show(ui, |ui| {
                ui.label("");
                for (ch, _) in meas {
                    ui.label(RichText::new(format!("CH{ch}")).font(FontId::new(13.0, theme::mono_bold())).color(theme::channel(*ch)));
                }
                ui.end_row();
                type Row = (&'static str, fn(&Measurements) -> String);
                let rows: [Row; 8] = [
                    ("Frequency", |m| si_opt(m.freq, "Hz")),
                    ("Period", |m| si_opt(m.period, "s")),
                    ("Duty", |m| m.duty.map_or("--".into(), |d| format!("{d:.1}%"))),
                    ("Vpp", |m| si(m.vpp, "V")),
                    ("Vmax", |m| si(m.vmax, "V")),
                    ("Vmin", |m| si(m.vmin, "V")),
                    ("Vavg", |m| si(m.vavg, "V")),
                    ("Vrms", |m| si(m.vrms, "V")),
                ];
                for (name, get) in rows {
                    ui.label(RichText::new(name).color(theme::MUTED).size(13.0));
                    for (_, m) in meas {
                        ui.monospace(get(m));
                    }
                    ui.end_row();
                }
            });
        });

        widgets::card(ui, |ui| {
            widgets::title(ui, "Cursors");
            let c = &mut self.screen.cursors;
            ui.horizontal(|ui| {
                for (mode, name) in [(CursorMode::Off, "Off"), (CursorMode::Track, "Track"), (CursorMode::Voltage, "Voltage")] {
                    if widgets::chip(ui, c.mode == mode, name).clicked() {
                        c.mode = mode;
                    }
                }
            });
            match c.mode {
                CursorMode::Off => widgets::caption(ui, "Track reads both channels at two real samples; drag the dashed lines."),
                CursorMode::Track => {
                    if let Some(([ta, tb], chans)) = screen::track_readout(f, c, self.prefs.view) {
                        let dt = (tb - ta).abs();
                        ui.spacing_mut().interact_size.y = 18.0;
                        egui::Grid::new("track").num_columns(3).spacing([14.0, 3.0]).show(ui, |ui| {
                            kv_row(ui, "", "A", "B", theme::MUTED);
                            kv_row(ui, "Time", &si(ta, "s"), &si(tb, "s"), theme::CURSOR);
                            for (ch, [va, vb]) in &chans {
                                kv_row(ui, &format!("CH{ch}"), &si(*va, "V"), &si(*vb, "V"), theme::channel(*ch));
                            }
                            kv_row(ui, "dt", &si(dt, "s"), &if dt > 0.0 { si(1.0 / dt, "Hz") } else { "--".into() }, theme::CURSOR);
                            for (ch, [va, vb]) in &chans {
                                kv_row(ui, &format!("CH{ch} dV"), &si(vb - va, "V"), "", theme::channel(*ch));
                            }
                        });
                    }
                }
                CursorMode::Voltage => {
                    ui.horizontal(|ui| {
                        for ch in 1..=2 {
                            if widgets::chip_colored(ui, c.channel == ch, &format!("CH{ch}"), Some(theme::channel(ch))).clicked() {
                                c.channel = ch;
                            }
                        }
                    });
                    widgets::caption(ui, "Drag the dashed lines on the screen.");
                }
            }
        });

        widgets::card(ui, |ui| {
            ui.horizontal(|ui| {
                widgets::title(ui, "Markers");
                if !self.screen.markers.is_empty() {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if widgets::button(ui, None, "Clear", Kind::Outline).clicked() {
                            self.screen.markers.clear();
                        }
                    });
                }
            });
            if self.screen.markers.is_empty() {
                widgets::caption(ui, "Double-click a trace to mark a sample. Right-click a marker to remove it.");
            }
            let mut remove = None;
            for (n, m) in self.screen.markers.iter().enumerate().take(8) {
                ui.horizontal(|ui| {
                    let v = screen::volts_at(f, m.channel, m.index).map(|v| si(v, "V")).unwrap_or_default();
                    ui.label(RichText::new(format!("M{}", n + 1)).font(FontId::new(13.0, theme::mono_bold())));
                    ui.label(RichText::new(format!("CH{}", m.channel)).monospace().color(theme::channel(m.channel)));
                    ui.monospace(format!("{}  {v}", si(screen::time_of(f, m.index), "s")));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if widgets::icon_button(ui, Icon::Close, "Remove", 26.0).clicked() {
                            remove = Some(n);
                        }
                    });
                });
            }
            if let Some(n) = remove {
                self.screen.markers.remove(n);
            }
        });
    }

    fn settings_dialog(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let frame = egui::Frame::new()
            .fill(theme::SURFACE)
            .stroke(Stroke::new(1.0, theme::LINE_STRONG))
            .corner_radius(CornerRadius::same(theme::RADIUS))
            .inner_margin(Margin::same(28));
        let mut done = false;
        let resp = egui::Modal::new(egui::Id::unique("husbb_settings")).frame(frame).show(ctx, |ui| {
            ui.set_width(480.0);
            ui.spacing_mut().item_spacing = vec2(10.0, 12.0);
            ui.label(RichText::new("Settings").font(FontId::new(17.0, theme::semibold())));
            ui.label(RichText::new("Export folder").font(FontId::new(14.0, theme::medium())));
            ui.horizontal(|ui| {
                let text = self.prefs.export_dir.as_ref().map_or("Not set".to_string(), |d| d.display().to_string());
                ui.add_sized([340.0, 28.0], egui::Label::new(RichText::new(text).monospace().color(theme::MUTED)).truncate());
                if widgets::button(ui, Some(Icon::SaveAs), "Choose", Kind::Outline).clicked()
                    && let Some(dir) = rfd::FileDialog::new().pick_folder()
                {
                    self.prefs.export_dir = Some(dir);
                }
            });
            widgets::caption(ui, "Export saves the current view here in one click, named HUSBB_DSO2D15_<date>_<time>.");
            switch_row(ui, "Save the screen as PNG", &mut self.prefs.export_png);
            switch_row(ui, "Save the data as CSV", &mut self.prefs.export_csv);
            switch_row(ui, "Write measurements on the screen", &mut self.prefs.overlay);
            ui.add_space(6.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::button(ui, None, "Done", Kind::Primary).clicked() {
                    done = true;
                }
            });
        });
        if done || resp.should_close() {
            self.settings_open = false;
        }
    }
}

fn coupling(s: &husbb::scope::Settings, ch: usize) -> &str {
    if s.coupling[ch - 1].is_empty() { "--" } else { &s.coupling[ch - 1] }
}

fn probe(s: &husbb::scope::Settings, ch: usize) -> String {
    let p = s.probe[ch - 1];
    if p > 0.0 { format!("{p}x") } else { "--".into() }
}

fn kv_row(ui: &mut Ui, key: &str, a: &str, b: &str, color: Color32) {
    ui.label(RichText::new(key).color(theme::MUTED).size(12.5));
    ui.label(RichText::new(a).monospace().color(color));
    ui.label(RichText::new(b).monospace().color(color));
    ui.end_row();
}

fn switch_row(ui: &mut Ui, label: &str, value: &mut bool) {
    ui.horizontal(|ui| {
        widgets::switch(ui, value);
        ui.label(label);
    });
}

fn status_bar(ui: &mut Ui, shared: &Shared, frame: Option<&Frame>) {
    ui.horizontal_centered(|ui| {
        let mono = |t: String, c: Color32| RichText::new(t).font(FontId::new(12.0, egui::FontFamily::Monospace)).color(c);
        let sep = |ui: &mut Ui| ui.label(mono("|".into(), theme::LINE_STRONG));
        if let Some(f) = frame {
            let h = &f.acq.header;
            let s = &f.settings;
            for ch in 1..=2usize {
                if h.enabled[ch - 1] {
                    ui.label(mono(format!("CH{ch} {}/div {} {}", si(h.volts_per_div[ch - 1], "V"), coupling(s, ch), probe(s, ch)), theme::channel(ch)));
                    sep(ui);
                }
            }
            ui.label(mono(format!("{}/div  {}", si(s.timebase, "s"), si(h.sample_rate, "Sa/s")), theme::INK));
            sep(ui);
            let slope = match s.trigger_slope {
                Slope::Rising => "rising",
                Slope::Falling => "falling",
                Slope::Either => "either",
            };
            ui.label(mono(format!("Trigger CH{} {slope} {} {}", s.trigger_source, si(s.trigger_level, "V"), s.trigger_sweep.label()), theme::TRIGGER));
            sep(ui);
            ui.label(mono(format!("{:.1} records/s", shared.records_per_s), theme::MUTED));
        }
        if let Some(e) = &shared.last_error {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.label(mono(e.clone(), theme::MUTED)));
        }
    });
}

pub fn options_from_args() -> Options {
    let mut opts = Options { resource: None, screenshot: None, screenshot_after: Duration::from_secs(8), cursors: None, tab: Tab::Scope };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--resource" => opts.resource = args.next(),
            "--screenshot" => opts.screenshot = args.next().map(PathBuf::from),
            "--after" => {
                if let Some(s) = args.next().and_then(|s| s.parse::<f64>().ok()) {
                    opts.screenshot_after = Duration::from_secs_f64(s);
                }
            }
            "--cursors" => {
                opts.cursors = match args.next().as_deref() {
                    Some("track") => Some(CursorMode::Track),
                    Some("voltage") => Some(CursorMode::Voltage),
                    _ => None,
                }
            }
            "--tab" => {
                if args.next().as_deref() == Some("measure") {
                    opts.tab = Tab::Measure;
                }
            }
            _ => eprintln!("ignoring unknown argument {a}"),
        }
    }
    opts
}
