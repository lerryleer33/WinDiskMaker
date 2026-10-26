//! Small, presentation-only widgets shared by the main screen.

use eframe::egui::{
    self, Align2, Color32, CursorIcon, Pos2, Rect, Response, RichText, Sense, Stroke, Ui, Vec2,
};

use crate::theme::{body, display, Palette};

const ROW_RADIUS: f32 = 7.0;

pub fn section_label(ui: &mut Ui, p: &Palette, text: &str) {
    ui.add_space(18.0);
    ui.label(RichText::new(text).font(display(13.0)).color(p.text));
    ui.add_space(6.0);
}

pub fn group<R>(ui: &mut Ui, p: &Palette, add: impl FnOnce(&mut Ui) -> R) -> R {
    let width = ui.available_width();
    egui::Frame::none()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.group_stroke))
        .rounding(10.0)
        .show(ui, |ui| {
            ui.set_width(width);
            add(ui)
        })
        .inner
}

pub fn separator(ui: &mut Ui, p: &Palette) {
    let rect = ui.available_rect_before_wrap();
    ui.painter().hline(
        (rect.left() + 16.0)..=(rect.right() - 16.0),
        rect.top() + 0.5,
        Stroke::new(1.0, p.separator),
    );
    ui.add_space(1.0);
}

pub fn app_mark(ui: &mut Ui, p: &Palette) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 8.0, p.accent);

    let body_rect = Rect::from_center_size(rect.center(), Vec2::new(14.0, 9.0));
    let white = Color32::WHITE;
    painter.rect_stroke(body_rect, 2.0, Stroke::new(1.4, white));
    painter.circle_filled(
        Pos2::new(body_rect.right() - 3.0, body_rect.center().y),
        1.1,
        white,
    );
    painter.line_segment(
        [
            Pos2::new(body_rect.left() + 2.5, body_rect.center().y),
            Pos2::new(body_rect.left() + 6.0, body_rect.center().y),
        ],
        Stroke::new(1.2, white),
    );
}

pub fn file_icon(ui: &mut Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
    let painter = ui.painter();
    let page = Rect::from_center_size(rect.center(), Vec2::new(13.0, 16.0));

    painter.rect_stroke(page, 2.0, Stroke::new(1.3, color));
    painter.line_segment(
        [
            Pos2::new(page.right() - 4.0, page.top()),
            Pos2::new(page.right() - 4.0, page.top() + 4.0),
        ],
        Stroke::new(1.3, color),
    );
    painter.line_segment(
        [
            Pos2::new(page.right() - 4.0, page.top() + 4.0),
            Pos2::new(page.right(), page.top() + 4.0),
        ],
        Stroke::new(1.3, color),
    );
    painter.line_segment(
        [
            Pos2::new(page.left() + 3.0, page.center().y + 1.0),
            Pos2::new(page.right() - 3.0, page.center().y + 1.0),
        ],
        Stroke::new(1.1, color),
    );
    painter.line_segment(
        [
            Pos2::new(page.left() + 3.0, page.center().y + 4.0),
            Pos2::new(page.right() - 5.0, page.center().y + 4.0),
        ],
        Stroke::new(1.1, color),
    );
}

pub fn status(ui: &mut Ui, p: &Palette, color: Color32, label: &str) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), body(12.0), p.text_secondary);
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(13.0 + galley.size().x, 18.0), Sense::hover());
    ui.painter()
        .circle_filled(Pos2::new(rect.left() + 4.0, rect.center().y), 4.0, color);
    ui.painter().galley(
        Pos2::new(rect.left() + 13.0, rect.center().y - galley.size().y / 2.0),
        galley,
        p.text_secondary,
    );
    response
}

pub fn primary_button(ui: &mut Ui, p: &Palette, label: &str, enabled: bool) -> Response {
    button(ui, p, label, enabled, ButtonKind::Primary)
}

pub fn secondary_button(ui: &mut Ui, p: &Palette, label: &str, enabled: bool) -> Response {
    button(ui, p, label, enabled, ButtonKind::Secondary)
}

pub fn plain_button(ui: &mut Ui, p: &Palette, label: &str, enabled: bool) -> Response {
    button(ui, p, label, enabled, ButtonKind::Plain)
}

pub fn danger_button(ui: &mut Ui, p: &Palette, label: &str) -> Response {
    button(ui, p, label, true, ButtonKind::Danger)
}

#[derive(Clone, Copy)]
enum ButtonKind {
    Primary,
    Secondary,
    Plain,
    Danger,
}

fn button(ui: &mut Ui, p: &Palette, label: &str, enabled: bool, kind: ButtonKind) -> Response {
    let text_color = match kind {
        ButtonKind::Primary | ButtonKind::Danger if enabled => Color32::WHITE,
        ButtonKind::Plain if enabled => p.accent,
        ButtonKind::Secondary if enabled => p.text,
        _ => p.text_tertiary,
    };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), body(13.0), text_color);
    let padding = match kind {
        ButtonKind::Plain => 8.0,
        _ => 14.0,
    };
    let size = Vec2::new(galley.size().x + padding * 2.0, 32.0);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);

    let hovered = enabled && response.hovered();
    let pressed = enabled && response.is_pointer_button_down_on();
    let painter = ui.painter();

    match kind {
        ButtonKind::Primary => {
            let fill = if !enabled {
                p.disabled
            } else if pressed {
                p.accent_pressed
            } else if hovered {
                p.accent_hover
            } else {
                p.accent
            };
            painter.rect_filled(rect, 6.0, fill);
        }
        ButtonKind::Secondary => {
            painter.rect_filled(
                rect,
                6.0,
                if !enabled {
                    p.disabled
                } else if hovered {
                    p.control_hover
                } else {
                    p.control
                },
            );
            if enabled {
                painter.rect_stroke(rect, 6.0, Stroke::new(1.0, p.control_stroke));
            }
        }
        ButtonKind::Plain => {
            if hovered {
                painter.rect_filled(rect, 6.0, p.control_hover);
            }
        }
        ButtonKind::Danger => {
            let fill = if pressed {
                Color32::from_rgb(180, 0, 18)
            } else if hovered {
                Color32::from_rgb(232, 24, 44)
            } else {
                p.danger
            };
            painter.rect_filled(rect, 6.0, fill);
        }
    }

    painter.galley(rect.center() - galley.size() / 2.0, galley, text_color);

    if enabled {
        response.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        response
    }
}

pub fn drive_row(
    ui: &mut Ui,
    p: &Palette,
    title: &str,
    meta: &str,
    tag: &str,
    tag_fg: Color32,
    tag_bg: Color32,
    selected: bool,
) -> Response {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 54.0), Sense::click());
    let highlight = rect.shrink2(Vec2::new(6.0, 3.0));

    if selected {
        ui.painter()
            .rect_filled(highlight, ROW_RADIUS, p.accent_soft);
    } else if response.hovered() {
        ui.painter().rect_filled(highlight, ROW_RADIUS, p.hover_row);
    }

    let center_y = rect.center().y;
    let radio = Pos2::new(rect.left() + 26.0, center_y);
    let ring = if selected { p.accent } else { p.text_tertiary };
    ui.painter()
        .circle_stroke(radio, 7.0, Stroke::new(1.5, ring));
    if selected {
        ui.painter().circle_filled(radio, 4.0, p.accent);
    }

    let tag_galley = ui
        .painter()
        .layout_no_wrap(tag.to_owned(), display(10.5), tag_fg);
    let tag_width = tag_galley.size().x + 16.0;
    let tag_rect = Rect::from_center_size(
        Pos2::new(rect.right() - 16.0 - tag_width / 2.0, center_y),
        Vec2::new(tag_width, 18.0),
    );
    ui.painter().rect_filled(tag_rect, 9.0, tag_bg);
    ui.painter().galley(
        tag_rect.center() - tag_galley.size() / 2.0,
        tag_galley,
        tag_fg,
    );

    let text_left = rect.left() + 44.0;
    let text_right = tag_rect.left() - 12.0;
    let clip = Rect::from_min_max(
        Pos2::new(text_left, rect.top()),
        Pos2::new(text_right, rect.bottom()),
    );
    let painter = ui.painter().with_clip_rect(clip);
    let title_galley = painter.layout_no_wrap(title.to_owned(), display(13.0), p.text);
    painter.galley(Pos2::new(text_left, center_y - 15.0), title_galley, p.text);
    let meta_galley = painter.layout_no_wrap(meta.to_owned(), body(11.5), p.text_secondary);
    painter.galley(
        Pos2::new(text_left, center_y + 2.0),
        meta_galley,
        p.text_secondary,
    );

    response.on_hover_cursor(CursorIcon::PointingHand)
}

pub fn option_row(
    ui: &mut Ui,
    p: &Palette,
    title: &str,
    caption: &str,
    value: &mut bool,
) -> Response {
    let width = ui.available_width();
    let (rect, mut response) = ui.allocate_exact_size(Vec2::new(width, 58.0), Sense::click());
    let highlight = rect.shrink2(Vec2::new(6.0, 3.0));

    if response.hovered() {
        ui.painter().rect_filled(highlight, ROW_RADIUS, p.hover_row);
    }
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }

    let switch_rect = Rect::from_center_size(
        Pos2::new(rect.right() - 16.0 - 19.0, rect.center().y),
        Vec2::new(38.0, 22.0),
    );
    let t = ui
        .ctx()
        .animate_bool_with_time(response.id.with("switch"), *value, 0.12);
    paint_switch(ui.painter(), p, switch_rect, t);

    let text_left = rect.left() + 16.0;
    let clip = Rect::from_min_max(
        Pos2::new(text_left, rect.top()),
        Pos2::new(switch_rect.left() - 14.0, rect.bottom()),
    );
    let painter = ui.painter().with_clip_rect(clip);
    let center_y = rect.center().y;
    let title_galley = painter.layout_no_wrap(title.to_owned(), display(13.0), p.text);
    painter.galley(Pos2::new(text_left, center_y - 15.0), title_galley, p.text);
    let caption_galley = painter.layout_no_wrap(caption.to_owned(), body(11.5), p.text_secondary);
    painter.galley(
        Pos2::new(text_left, center_y + 2.0),
        caption_galley,
        p.text_secondary,
    );

    response.on_hover_cursor(CursorIcon::PointingHand)
}

fn paint_switch(painter: &egui::Painter, p: &Palette, rect: Rect, t: f32) {
    let track = Rect::from_min_max(
        Pos2::new(rect.left(), rect.top() + 1.0),
        Pos2::new(rect.right(), rect.bottom() - 1.0),
    );
    let color = lerp_color(p.switch_off, p.accent, t);
    painter.rect_filled(track, 10.0, color);

    let knob_x = lerp(track.left() + 10.0, track.right() - 10.0, t);
    let knob = Pos2::new(knob_x, track.center().y);
    painter.circle_filled(knob, 8.5, Color32::WHITE);
}

pub fn progress_bar(ui: &mut Ui, p: &Palette, fraction: f32) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 4.0), Sense::hover());
    ui.painter().rect_filled(rect, 2.0, p.control);
    let filled_width = (rect.width() * fraction.clamp(0.0, 1.0)).max(0.0);
    if filled_width > 0.0 {
        let filled = Rect::from_min_size(rect.min, Vec2::new(filled_width, rect.height()));
        ui.painter().rect_filled(filled, 2.0, p.accent);
    }
}

pub fn stepper(ui: &mut Ui, p: &Palette, current: usize, labels: &[&str]) {
    let gap = 6.0;
    let width = (ui.available_width() - gap * (labels.len().saturating_sub(1) as f32))
        / labels.len() as f32;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for (index, label) in labels.iter().enumerate() {
            let step = index + 1;
            ui.allocate_ui_with_layout(
                Vec2::new(width, 30.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 3.0), Sense::hover());
                    let color = if step <= current { p.accent } else { p.control };
                    ui.painter().rect_filled(rect, 1.5, color);
                    ui.add_space(5.0);
                    let label_color = if step == current {
                        p.text
                    } else if step < current {
                        p.text_secondary
                    } else {
                        p.text_tertiary
                    };
                    ui.add(
                        egui::Label::new(RichText::new(*label).font(body(10.5)).color(label_color))
                            .truncate(true),
                    );
                },
            );
        }
    });
}

pub fn alert_badge(ui: &mut Ui, color: Color32, background: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), 16.0, background);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        "!",
        display(16.0),
        color,
    );
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgb(
        lerp(a.r() as f32, b.r() as f32, t).round() as u8,
        lerp(a.g() as f32, b.g() as f32, t).round() as u8,
        lerp(a.b() as f32, b.b() as f32, t).round() as u8,
    )
}
