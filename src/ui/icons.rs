//! 用 Painter 手绘的小图标，避免依赖字体里的表情符号。

use egui::{Color32, CornerRadius, Pos2, Rect, Stroke, StrokeKind, Ui, Vec2};

use super::theme;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    Folder,
    FolderUp,
    Archive,
    File,
    Locked,
}

pub fn paint_in(ui: &Ui, rect: Rect, kind: IconKind) {
    let p = ui.painter();
    let r = Rect::from_center_size(rect.center(), Vec2::splat(rect.height().min(rect.width())));
    let w = r.width();
    let x = r.left();
    let y = r.top();

    match kind {
        IconKind::Folder | IconKind::FolderUp => {
            let body = Rect::from_min_max(
                Pos2::new(x + w * 0.06, y + w * 0.28),
                Pos2::new(x + w * 0.94, y + w * 0.86),
            );
            let tab = Rect::from_min_max(
                Pos2::new(x + w * 0.06, y + w * 0.16),
                Pos2::new(x + w * 0.48, y + w * 0.30),
            );
            p.rect_filled(tab, CornerRadius::same(2), theme::FOLDER_DARK);
            p.rect_filled(body, CornerRadius::same(2), theme::FOLDER);
            p.rect_stroke(
                body,
                CornerRadius::same(2),
                Stroke::new(1.0, theme::FOLDER_DARK),
                StrokeKind::Inside,
            );
            if kind == IconKind::FolderUp {
                let c = body.center();
                let s = w * 0.20;
                p.add(egui::Shape::convex_polygon(
                    vec![
                        Pos2::new(c.x, c.y - s),
                        Pos2::new(c.x - s, c.y + s * 0.7),
                        Pos2::new(c.x + s, c.y + s * 0.7),
                    ],
                    Color32::WHITE,
                    Stroke::NONE,
                ));
            }
        }
        IconKind::Archive => {
            let body = Rect::from_min_max(
                Pos2::new(x + w * 0.14, y + w * 0.14),
                Pos2::new(x + w * 0.86, y + w * 0.88),
            );
            p.rect_filled(body, CornerRadius::same(2), theme::ARCHIVE);
            p.rect_stroke(
                body,
                CornerRadius::same(2),
                Stroke::new(1.0, theme::ARCHIVE_DARK),
                StrokeKind::Inside,
            );
            // 中间的“拉链”竖条
            let band = Rect::from_min_max(
                Pos2::new(x + w * 0.44, y + w * 0.14),
                Pos2::new(x + w * 0.56, y + w * 0.88),
            );
            p.rect_filled(band, CornerRadius::ZERO, Color32::from_rgb(0xff, 0xf3, 0xd0));
            for i in 0..4 {
                let ty = y + w * (0.24 + i as f32 * 0.16);
                p.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(x + w * 0.44, ty),
                        Pos2::new(x + w * 0.56, ty + w * 0.07),
                    ),
                    CornerRadius::ZERO,
                    theme::ARCHIVE_DARK,
                );
            }
        }
        IconKind::File => {
            let body = Rect::from_min_max(
                Pos2::new(x + w * 0.20, y + w * 0.12),
                Pos2::new(x + w * 0.82, y + w * 0.90),
            );
            p.rect_filled(body, CornerRadius::same(2), Color32::WHITE);
            p.rect_stroke(
                body,
                CornerRadius::same(2),
                Stroke::new(1.0, Color32::from_rgb(0xa8, 0xb2, 0xbe)),
                StrokeKind::Inside,
            );
            for i in 0..3 {
                let ly = y + w * (0.34 + i as f32 * 0.16);
                p.line_segment(
                    [
                        Pos2::new(x + w * 0.30, ly),
                        Pos2::new(x + w * 0.72, ly),
                    ],
                    Stroke::new(1.0, Color32::from_rgb(0xc4, 0xcc, 0xd6)),
                );
            }
        }
        IconKind::Locked => {
            let body = Rect::from_min_max(
                Pos2::new(x + w * 0.24, y + w * 0.44),
                Pos2::new(x + w * 0.76, y + w * 0.86),
            );
            p.rect_filled(body, CornerRadius::same(2), theme::LOCK);
            let shackle = Rect::from_min_max(
                Pos2::new(x + w * 0.34, y + w * 0.18),
                Pos2::new(x + w * 0.66, y + w * 0.52),
            );
            p.rect_stroke(
                shackle,
                CornerRadius::same(6),
                Stroke::new(w * 0.09, theme::LOCK),
                StrokeKind::Inside,
            );
        }
    }
}

/// 工具栏上使用的大图标按钮。
pub fn tool_button(ui: &mut Ui, label: &str, kind: ToolIcon, enabled: bool) -> egui::Response {
    let desired = Vec2::new(62.0, 54.0);
    let (rect, response) = ui.allocate_exact_size(
        desired,
        if enabled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );

    let hovered = response.hovered() && enabled;
    let down = response.is_pointer_button_down_on() && enabled;

    if down {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(7),
            theme::ACCENT_SOFT,
        );
    } else if hovered {
        ui.painter().rect_filled(rect, CornerRadius::same(7), theme::HOVER_BG);
    }

    let icon_rect = Rect::from_center_size(
        Pos2::new(rect.center().x, rect.top() + 17.0),
        Vec2::splat(24.0),
    );
    paint_tool_icon(ui, icon_rect, kind, enabled, hovered);

    let color = if enabled {
        if hovered || down { theme::ACCENT_HOVER } else { theme::TEXT }
    } else {
        Color32::from_rgb(0xaa, 0xb1, 0xba)
    };
    ui.painter().text(
        Pos2::new(rect.center().x, rect.bottom() - 12.0),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(11.5),
        color,
    );

    response
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToolIcon {
    Add,
    Extract,
    Test,
    View,
    Delete,
    Info,
    Up,
    Refresh,
    Home,
}

fn paint_tool_icon(ui: &Ui, r: Rect, kind: ToolIcon, enabled: bool, hovered: bool) {
    let p = ui.painter();
    let w = r.width();
    let c = r.center();
    let main = if enabled {
        if hovered { theme::ACCENT_HOVER } else { theme::ACCENT }
    } else {
        Color32::from_rgb(0xb4, 0xba, 0xc2)
    };
    // 统一的浅色填充，让图标呈现代的单色系
    let accent = if enabled {
        theme::ACCENT_SOFT
    } else {
        Color32::from_rgb(0xe8, 0xeb, 0xef)
    };

    match kind {
        ToolIcon::Add => {
            let body = Rect::from_min_max(
                Pos2::new(r.left() + w * 0.12, r.top() + w * 0.16),
                Pos2::new(r.right() - w * 0.28, r.bottom() - w * 0.12),
            );
            p.rect_filled(body, CornerRadius::same(2), accent);
            p.rect_stroke(
                body,
                CornerRadius::same(2),
                Stroke::new(1.2, main),
                StrokeKind::Inside,
            );
            let cx = r.right() - w * 0.18;
            let cy = r.bottom() - w * 0.20;
            let s = w * 0.20;
            p.line_segment(
                [Pos2::new(cx - s, cy), Pos2::new(cx + s, cy)],
                Stroke::new(2.6, theme::OK_GREEN),
            );
            p.line_segment(
                [Pos2::new(cx, cy - s), Pos2::new(cx, cy + s)],
                Stroke::new(2.6, theme::OK_GREEN),
            );
        }
        ToolIcon::Extract => {
            let body = Rect::from_min_max(
                Pos2::new(r.left() + w * 0.10, r.top() + w * 0.10),
                Pos2::new(r.right() - w * 0.10, r.top() + w * 0.46),
            );
            p.rect_filled(body, CornerRadius::same(2), accent);
            let s = w * 0.18;
            p.line_segment(
                [
                    Pos2::new(c.x, r.top() + w * 0.50),
                    Pos2::new(c.x, r.bottom() - w * 0.22),
                ],
                Stroke::new(2.4, main),
            );
            p.add(egui::Shape::convex_polygon(
                vec![
                    Pos2::new(c.x, r.bottom() - w * 0.04),
                    Pos2::new(c.x - s, r.bottom() - w * 0.28),
                    Pos2::new(c.x + s, r.bottom() - w * 0.28),
                ],
                main,
                Stroke::NONE,
            ));
        }
        ToolIcon::Test => {
            p.circle_stroke(c, w * 0.36, Stroke::new(2.0, main));
            p.line_segment(
                [
                    Pos2::new(c.x - w * 0.16, c.y + w * 0.02),
                    Pos2::new(c.x - w * 0.03, c.y + w * 0.16),
                ],
                Stroke::new(2.4, theme::OK_GREEN),
            );
            p.line_segment(
                [
                    Pos2::new(c.x - w * 0.03, c.y + w * 0.16),
                    Pos2::new(c.x + w * 0.20, c.y - w * 0.16),
                ],
                Stroke::new(2.4, theme::OK_GREEN),
            );
        }
        ToolIcon::View => {
            p.circle_stroke(c, w * 0.30, Stroke::new(2.0, main));
            p.line_segment(
                [
                    Pos2::new(c.x + w * 0.22, c.y + w * 0.22),
                    Pos2::new(c.x + w * 0.40, c.y + w * 0.40),
                ],
                Stroke::new(2.6, main),
            );
        }
        ToolIcon::Delete => {
            let body = Rect::from_min_max(
                Pos2::new(c.x - w * 0.26, r.top() + w * 0.24),
                Pos2::new(c.x + w * 0.26, r.bottom() - w * 0.10),
            );
            p.rect_filled(body, CornerRadius::same(2), Color32::from_rgb(0xd9, 0x6b, 0x5a));
            p.rect_filled(
                Rect::from_min_max(
                    Pos2::new(c.x - w * 0.34, r.top() + w * 0.12),
                    Pos2::new(c.x + w * 0.34, r.top() + w * 0.24),
                ),
                CornerRadius::same(2),
                Color32::from_rgb(0xb4, 0x4a, 0x3a),
            );
            for i in 0..2 {
                let lx = c.x + (i as f32 - 0.5) * w * 0.24;
                p.line_segment(
                    [
                        Pos2::new(lx, r.top() + w * 0.34),
                        Pos2::new(lx, r.bottom() - w * 0.20),
                    ],
                    Stroke::new(1.6, Color32::from_rgb(0xff, 0xe3, 0xdc)),
                );
            }
        }
        ToolIcon::Info => {
            p.circle_stroke(c, w * 0.36, Stroke::new(2.0, main));
            p.circle_filled(Pos2::new(c.x, c.y - w * 0.17), w * 0.05, main);
            p.line_segment(
                [
                    Pos2::new(c.x, c.y - w * 0.04),
                    Pos2::new(c.x, c.y + w * 0.20),
                ],
                Stroke::new(2.2, main),
            );
        }
        ToolIcon::Up => {
            let s = w * 0.22;
            p.add(egui::Shape::convex_polygon(
                vec![
                    Pos2::new(c.x, c.y - w * 0.30),
                    Pos2::new(c.x - s, c.y - w * 0.02),
                    Pos2::new(c.x + s, c.y - w * 0.02),
                ],
                main,
                Stroke::NONE,
            ));
            p.line_segment(
                [
                    Pos2::new(c.x, c.y - w * 0.06),
                    Pos2::new(c.x, c.y + w * 0.30),
                ],
                Stroke::new(2.4, main),
            );
        }
        ToolIcon::Refresh => {
            p.circle_stroke(c, w * 0.30, Stroke::new(2.2, main));
            p.rect_filled(
                Rect::from_min_max(
                    Pos2::new(c.x + w * 0.04, c.y - w * 0.42),
                    Pos2::new(c.x + w * 0.42, c.y - w * 0.10),
                ),
                CornerRadius::ZERO,
                theme::PANEL_BG,
            );
            let s = w * 0.16;
            p.add(egui::Shape::convex_polygon(
                vec![
                    Pos2::new(c.x + w * 0.30, c.y - w * 0.36),
                    Pos2::new(c.x + w * 0.30 - s, c.y - w * 0.12),
                    Pos2::new(c.x + w * 0.30 + s, c.y - w * 0.12),
                ],
                main,
                Stroke::NONE,
            ));
        }
        ToolIcon::Home => {
            let s = w * 0.34;
            p.add(egui::Shape::convex_polygon(
                vec![
                    Pos2::new(c.x, c.y - s),
                    Pos2::new(c.x - s, c.y),
                    Pos2::new(c.x + s, c.y),
                ],
                main,
                Stroke::NONE,
            ));
            p.rect_filled(
                Rect::from_min_max(
                    Pos2::new(c.x - s * 0.66, c.y),
                    Pos2::new(c.x + s * 0.66, c.y + s * 0.8),
                ),
                CornerRadius::same(1),
                accent,
            );
        }
    }
}
