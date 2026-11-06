//! The "Bugra" design system (dark theme) mapped onto egui, plus the DSO2D15's own trace colours.
//!
//! Bugra: warm black surfaces, one green accent (Android green), Geist for text and JetBrains Mono
//! for numbers, pill buttons and chips, hairline borders instead of shadows.

use std::collections::BTreeMap;
use std::sync::Arc;

use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle, vec2};

// --- Bugra tokens (dark) ---------------------------------------------------------------------

pub const BG: Color32 = Color32::from_rgb(0x13, 0x11, 0x10);
pub const SURFACE: Color32 = Color32::from_rgb(0x1b, 0x19, 0x17);
pub const SURFACE_2: Color32 = Color32::from_rgb(0x24, 0x21, 0x1e);
pub const INK: Color32 = Color32::from_rgb(0xf4, 0xf1, 0xec);
pub const MUTED: Color32 = Color32::from_rgb(0xa2, 0x9b, 0x91);
/// rgba(244,241,236,0.09), premultiplied
pub const LINE: Color32 = Color32::from_rgba_premultiplied(22, 22, 21, 23);
/// rgba(244,241,236,0.18), premultiplied
pub const LINE_STRONG: Color32 = Color32::from_rgba_premultiplied(44, 43, 42, 46);
pub const BORDER_CONTROL: Color32 = Color32::from_rgb(0x85, 0x7e, 0x74);
pub const ACCENT: Color32 = Color32::from_rgb(0x3d, 0xdc, 0x84);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(0x6f, 0xf0, 0xa8);
/// rgba(61,220,132,0.13), premultiplied
pub const ACCENT_QUIET: Color32 = Color32::from_rgba_premultiplied(8, 29, 17, 33);
pub const ON_ACCENT: Color32 = Color32::from_rgb(0x13, 0x11, 0x10);
pub const CODE_BG: Color32 = Color32::from_rgb(0x0c, 0x0b, 0x0a);
pub const OK: Color32 = Color32::from_rgb(0x7f, 0xc9, 0x8a);
pub const WARN: Color32 = Color32::from_rgb(0xe8, 0x70, 0x5a);
pub const INFO: Color32 = Color32::from_rgb(0x8a, 0xb8, 0xe8);
/// rgba(244,241,236,0.38), premultiplied
pub const DISABLED_INK: Color32 = Color32::from_rgba_premultiplied(93, 92, 90, 97);

pub const RADIUS: u8 = 14;
pub const RADIUS_SM: u8 = 10;
pub const CONTROL_H: f32 = 32.0;
pub const PILL: u8 = 16;

// --- the scope's own colours (firmware `phoenix`, ARGB table at 0xcb1e8) ---------------------

pub const CH1: Color32 = Color32::from_rgb(0xff, 0xff, 0x00);
pub const CH2: Color32 = Color32::from_rgb(0x00, 0xff, 0x00);
pub const TRIGGER: Color32 = Color32::from_rgb(0xff, 0x80, 0x00);
pub const SCREEN_BG: Color32 = CODE_BG;
pub const GRID: Color32 = Color32::from_rgba_premultiplied(38, 38, 37, 40);
pub const GRID_AXIS: Color32 = Color32::from_rgba_premultiplied(76, 75, 74, 80);
pub const CURSOR: Color32 = INFO;

pub fn channel(ch: usize) -> Color32 {
    if ch == 2 { CH2 } else { CH1 }
}

pub fn medium() -> FontFamily {
    FontFamily::Name("geist-medium".into())
}

pub fn semibold() -> FontFamily {
    FontFamily::Name("geist-semibold".into())
}

pub fn mono_bold() -> FontFamily {
    FontFamily::Name("mono-semibold".into())
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let mut add = |name: &str, bytes: &'static [u8]| {
        fonts.font_data.insert(name.into(), Arc::new(FontData::from_static(bytes)));
    };
    add("geist", include_bytes!("../../assets/fonts/Geist-Regular.ttf"));
    add("geist-medium", include_bytes!("../../assets/fonts/Geist-Medium.ttf"));
    add("geist-semibold", include_bytes!("../../assets/fonts/Geist-SemiBold.ttf"));
    add("mono", include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf"));
    add("mono-semibold", include_bytes!("../../assets/fonts/JetBrainsMono-SemiBold.ttf"));
    let fallback_prop = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let fallback_mono = fonts.families.get(&FontFamily::Monospace).cloned().unwrap_or_default();
    let with = |first: &str, rest: &[String]| std::iter::once(first.to_string()).chain(rest.iter().cloned()).collect::<Vec<_>>();
    fonts.families.insert(FontFamily::Proportional, with("geist", &fallback_prop));
    fonts.families.insert(FontFamily::Monospace, with("mono", &fallback_mono));
    fonts.families.insert(medium(), with("geist-medium", &fallback_prop));
    fonts.families.insert(semibold(), with("geist-semibold", &fallback_prop));
    fonts.families.insert(mono_bold(), with("mono-semibold", &fallback_mono));
    ctx.set_fonts(fonts);

    ctx.global_style_mut(|style| {
        style.text_styles = BTreeMap::from([
            (TextStyle::Heading, FontId::new(15.0, semibold())),
            (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(13.5, medium())),
            (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
        ]);
        let sp = &mut style.spacing;
        sp.item_spacing = vec2(8.0, 8.0);
        sp.button_padding = vec2(14.0, 6.0);
        sp.interact_size.y = 28.0;
        sp.combo_height = 240.0;

        let v = &mut style.visuals;
        v.dark_mode = true;
        v.override_text_color = Some(INK);
        v.weak_text_color = Some(MUTED);
        v.panel_fill = BG;
        v.window_fill = SURFACE;
        v.window_stroke = Stroke::new(1.0, LINE_STRONG);
        v.window_corner_radius = CornerRadius::same(RADIUS);
        v.menu_corner_radius = CornerRadius::same(RADIUS_SM);
        v.extreme_bg_color = CODE_BG;
        v.faint_bg_color = SURFACE_2;
        v.code_bg_color = CODE_BG;
        v.warn_fg_color = WARN;
        v.error_fg_color = WARN;
        v.hyperlink_color = ACCENT;
        v.selection.bg_fill = ACCENT_QUIET;
        v.selection.stroke = Stroke::new(1.0, ACCENT);
        v.striped = true;

        let w = &mut v.widgets;
        w.noninteractive.bg_fill = SURFACE;
        w.noninteractive.weak_bg_fill = SURFACE;
        w.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
        w.noninteractive.fg_stroke = Stroke::new(1.0, INK);
        w.noninteractive.corner_radius = CornerRadius::same(RADIUS_SM);
        for (state, fill, stroke) in [
            (&mut w.inactive, Color32::TRANSPARENT, LINE_STRONG),
            (&mut w.hovered, SURFACE_2, INK),
            (&mut w.active, SURFACE_2, INK),
            (&mut w.open, SURFACE_2, LINE_STRONG),
        ] {
            state.weak_bg_fill = fill;
            state.bg_fill = SURFACE_2;
            state.bg_stroke = Stroke::new(1.0, stroke);
            state.fg_stroke = Stroke::new(1.2, INK);
            state.corner_radius = CornerRadius::same(PILL);
            state.expansion = 0.0;
        }
        w.inactive.bg_stroke = Stroke::new(1.0, BORDER_CONTROL); // checkboxes and fields need 3:1
        w.inactive.weak_bg_fill = Color32::TRANSPARENT;
    });
}
