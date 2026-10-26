//! Design tokens, fonts, and egui styling for WinDisk Maker.
//!
//! The palette intentionally mirrors macOS system colors: neutral grays with a
//! single system-blue accent. Semantic colors are reserved for status meaning,
//! never decoration.

use eframe::egui::{
    self, epaint::Shadow, Color32, FontData, FontDefinitions, FontFamily, FontId, Rounding, Stroke,
    Style, TextStyle, Vec2, Visuals,
};

const SF_PRO: &str = "/System/Library/Fonts/SFNS.ttf";
const SF_MONO: &str = "/System/Library/Fonts/SFNSMono.ttf";
const HELVETICA_NEUE: &str = "/System/Library/Fonts/HelveticaNeue.ttc";
const MENLO: &str = "/System/Library/Fonts/Menlo.ttc";

pub const FAMILY_DISPLAY: &str = "display";

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    pub window: Color32,
    pub surface: Color32,
    pub inset: Color32,
    pub separator: Color32,
    pub group_stroke: Color32,
    pub text: Color32,
    pub text_secondary: Color32,
    pub text_tertiary: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_pressed: Color32,
    pub accent_soft: Color32,
    pub success: Color32,
    pub success_soft: Color32,
    pub warning: Color32,
    pub danger: Color32,
    pub danger_soft: Color32,
    pub control: Color32,
    pub control_hover: Color32,
    pub control_stroke: Color32,
    pub disabled: Color32,
    pub switch_off: Color32,
    pub hover_row: Color32,
    pub scrim: Color32,
    pub shadow: Color32,
}

impl Palette {
    /// Soft tints and separators are pre-blended solids. egui 0.27 blends in
    /// gamma space, so alpha colors render noticeably brighter than intended.
    pub fn for_mode(dark: bool) -> Self {
        if dark {
            Self {
                dark,
                window: Color32::from_rgb(28, 28, 30),
                surface: Color32::from_rgb(44, 44, 46),
                inset: Color32::from_rgb(35, 35, 37),
                separator: Color32::from_rgb(64, 64, 66),
                group_stroke: Color32::from_rgb(60, 60, 62),
                text: Color32::from_rgb(245, 245, 247),
                text_secondary: Color32::from_rgb(152, 152, 157),
                text_tertiary: Color32::from_rgb(110, 110, 115),
                accent: Color32::from_rgb(10, 132, 255),
                accent_hover: Color32::from_rgb(48, 150, 255),
                accent_pressed: Color32::from_rgb(0, 112, 224),
                accent_soft: Color32::from_rgb(37, 63, 92),
                success: Color32::from_rgb(48, 209, 88),
                success_soft: Color32::from_rgb(45, 77, 54),
                warning: Color32::from_rgb(255, 159, 10),
                danger: Color32::from_rgb(255, 69, 58),
                danger_soft: Color32::from_rgb(86, 49, 48),
                control: Color32::from_rgb(58, 58, 60),
                control_hover: Color32::from_rgb(70, 70, 73),
                control_stroke: Color32::from_rgb(86, 86, 88),
                disabled: Color32::from_rgb(52, 52, 54),
                switch_off: Color32::from_rgb(99, 99, 102),
                hover_row: Color32::from_rgb(57, 57, 58),
                scrim: Color32::from_black_alpha(130),
                shadow: Color32::from_black_alpha(110),
            }
        } else {
            Self {
                dark,
                window: Color32::from_rgb(245, 245, 247),
                surface: Color32::from_rgb(255, 255, 255),
                inset: Color32::from_rgb(242, 242, 244),
                separator: Color32::from_rgb(232, 232, 232),
                group_stroke: Color32::from_rgb(237, 237, 237),
                text: Color32::from_rgb(29, 29, 31),
                text_secondary: Color32::from_rgb(110, 110, 115),
                text_tertiary: Color32::from_rgb(154, 154, 160),
                accent: Color32::from_rgb(0, 122, 255),
                accent_hover: Color32::from_rgb(10, 111, 232),
                accent_pressed: Color32::from_rgb(0, 98, 204),
                accent_soft: Color32::from_rgb(230, 242, 255),
                success: Color32::from_rgb(36, 138, 61),
                success_soft: Color32::from_rgb(229, 241, 232),
                warning: Color32::from_rgb(178, 80, 0),
                danger: Color32::from_rgb(215, 0, 21),
                danger_soft: Color32::from_rgb(251, 230, 232),
                control: Color32::from_rgb(255, 255, 255),
                control_hover: Color32::from_rgb(240, 240, 242),
                control_stroke: Color32::from_rgb(214, 214, 214),
                disabled: Color32::from_rgb(236, 236, 238),
                switch_off: Color32::from_rgb(199, 199, 204),
                hover_row: Color32::from_rgb(245, 245, 245),
                scrim: Color32::from_black_alpha(88),
                shadow: Color32::from_black_alpha(60),
            }
        }
    }
}

pub fn body(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn display(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(FAMILY_DISPLAY.into()))
}

pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

fn font_at(path: &str, index: u32) -> Option<FontData> {
    std::fs::read(path).ok().map(|bytes| {
        let mut data = FontData::from_owned(bytes);
        data.index = index;
        data
    })
}

/// Loads system fonts once. Falls back to egui's bundled fonts on any failure.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    let body_loaded = if let Some(font) = font_at(SF_PRO, 0) {
        fonts.font_data.insert("wdm-body".to_owned(), font);
        true
    } else if let Some(font) = font_at(HELVETICA_NEUE, 0) {
        fonts.font_data.insert("wdm-body".to_owned(), font);
        true
    } else {
        false
    };

    // egui 0.27 has no synthetic bold, so emphasis uses a real medium face.
    let display_loaded = if let Some(font) = font_at(HELVETICA_NEUE, 10) {
        fonts.font_data.insert("wdm-display".to_owned(), font);
        true
    } else if let Some(font) = font_at(HELVETICA_NEUE, 1) {
        fonts.font_data.insert("wdm-display".to_owned(), font);
        true
    } else {
        false
    };

    let bold_loaded = if let Some(font) = font_at(HELVETICA_NEUE, 1) {
        fonts.font_data.insert("wdm-bold".to_owned(), font);
        true
    } else {
        false
    };

    let mono_loaded = if let Some(font) = font_at(SF_MONO, 0) {
        fonts.font_data.insert("wdm-mono".to_owned(), font);
        true
    } else if let Some(font) = font_at(MENLO, 0) {
        fonts.font_data.insert("wdm-mono".to_owned(), font);
        true
    } else {
        false
    };

    let defaults = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();

    if body_loaded {
        if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
            family.insert(0, "wdm-body".to_owned());
        }
    }

    if mono_loaded {
        if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
            family.insert(0, "wdm-mono".to_owned());
        }
    }

    let mut display_family = Vec::new();
    if display_loaded {
        display_family.push("wdm-display".to_owned());
    }
    if body_loaded {
        display_family.push("wdm-body".to_owned());
    }
    display_family.extend(defaults.iter().cloned());
    fonts
        .families
        .insert(FontFamily::Name(FAMILY_DISPLAY.into()), display_family);

    if bold_loaded {
        let mut bold_family = vec!["wdm-bold".to_owned()];
        if body_loaded {
            bold_family.push("wdm-body".to_owned());
        }
        bold_family.extend(defaults);
        fonts
            .families
            .insert(FontFamily::Name("bold".into()), bold_family);
    }

    ctx.set_fonts(fonts);
}

pub fn apply_style(ctx: &egui::Context, dark: bool) {
    let p = Palette::for_mode(dark);
    let mut style = Style::default();

    style.visuals = visuals(&p);
    style.animation_time = 0.12;
    style.interaction.selectable_labels = false;
    style.interaction.multi_widget_text_select = false;

    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(10.0, 5.0);
    style.spacing.interact_size = Vec2::new(0.0, 24.0);
    style.spacing.window_margin = egui::Margin::same(8.0);
    style.spacing.menu_margin = egui::Margin::same(6.0);
    style.spacing.scroll.floating = true;
    style.spacing.scroll.bar_width = 8.0;
    style.spacing.scroll.floating_width = 6.0;
    style.spacing.scroll.handle_min_length = 32.0;
    style.spacing.scroll.foreground_color = true;
    style.spacing.scroll.dormant_handle_opacity = 0.0;
    style.spacing.scroll.active_handle_opacity = 0.45;
    style.spacing.scroll.interact_handle_opacity = 0.75;

    style.text_styles = [
        (TextStyle::Heading, display(20.0)),
        (TextStyle::Body, body(13.0)),
        (TextStyle::Monospace, mono(11.5)),
        (TextStyle::Button, body(13.0)),
        (TextStyle::Small, body(11.0)),
    ]
    .into();

    ctx.set_style(style);
}

fn visuals(p: &Palette) -> Visuals {
    let transparent = Color32::TRANSPARENT;
    let mut v = if p.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    v.dark_mode = p.dark;
    v.override_text_color = Some(p.text);
    v.panel_fill = p.window;
    v.window_fill = p.surface;
    v.extreme_bg_color = p.inset;
    v.faint_bg_color = p.inset;
    v.code_bg_color = p.inset;
    v.window_stroke = Stroke::new(0.0, transparent);
    v.window_rounding = Rounding::same(12.0);
    v.menu_rounding = Rounding::same(8.0);
    v.selection.bg_fill = p.accent_soft;
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.hyperlink_color = p.accent;
    v.striped = false;

    v.window_shadow = Shadow {
        offset: Vec2::new(0.0, 10.0),
        blur: 30.0,
        spread: 0.0,
        color: p.shadow,
    };
    v.popup_shadow = Shadow {
        offset: Vec2::new(0.0, 6.0),
        blur: 18.0,
        spread: 0.0,
        color: p.shadow,
    };

    v.widgets.noninteractive.bg_fill = p.surface;
    v.widgets.noninteractive.weak_bg_fill = p.surface;
    v.widgets.noninteractive.bg_stroke = Stroke::new(0.0, transparent);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.noninteractive.rounding = Rounding::same(6.0);
    v.widgets.noninteractive.expansion = 0.0;

    v.widgets.inactive.bg_fill = p.control;
    v.widgets.inactive.weak_bg_fill = p.control;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.control_stroke);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.inactive.rounding = Rounding::same(6.0);
    v.widgets.inactive.expansion = 0.0;

    v.widgets.hovered.bg_fill = p.control_hover;
    v.widgets.hovered.weak_bg_fill = p.control_hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.control_stroke);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.hovered.rounding = Rounding::same(6.0);
    v.widgets.hovered.expansion = 0.0;

    v.widgets.active.bg_fill = p.accent_soft;
    v.widgets.active.weak_bg_fill = p.accent_soft;
    v.widgets.active.bg_stroke = Stroke::new(1.0, p.accent);
    v.widgets.active.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.active.rounding = Rounding::same(6.0);
    v.widgets.active.expansion = 0.0;

    v.widgets.open = v.widgets.active;
    v
}
