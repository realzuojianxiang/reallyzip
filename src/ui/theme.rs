//! 中文字体加载与整体视觉风格。
//!
//! 设计语言：现代、克制、专业的浅色界面。
//! - 主色采用沉稳的靛蓝(Indigo)，用于强调与主操作。
//! - 背景分三层：窗口基底、内容面板、浮层/工具栏，用细微灰阶区分层次。
//! - 元素统一使用圆角(6/8px)与轻投影，营造卡片化、精致的质感。

use egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Shadow, Stroke,
};
use std::sync::Arc;

const CJK_CANDIDATES: &[&str] = &[
    "C:/Windows/Fonts/msyh.ttc",
    "C:/Windows/Fonts/msyhl.ttc",
    "C:/Windows/Fonts/simhei.ttf",
    "C:/Windows/Fonts/Deng.ttf",
    "C:/Windows/Fonts/simsun.ttc",
    "/System/Library/Fonts/PingFang.ttc",
    "/System/Library/Fonts/STHeiti Light.ttc",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
];

pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);
    install_style(ctx);
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    for path in CJK_CANDIDATES {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        fonts
            .font_data
            .insert("cjk".to_owned(), Arc::new(FontData::from_owned(bytes)));
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "cjk".to_owned());
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .push("cjk".to_owned());
        break;
    }

    ctx.set_fonts(fonts);
}

// ------------------------------------------------------------------ 调色板

// 主色系（靛蓝）
pub const ACCENT: Color32 = Color32::from_rgb(0x3b, 0x6f, 0xe0);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(0x35, 0x63, 0xcc);
pub const ACCENT_DARK: Color32 = Color32::from_rgb(0x2c, 0x4f, 0xa8);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(0xe3, 0xec, 0xfb);
pub const ACCENT_SOFTER: Color32 = Color32::from_rgb(0xf0, 0xf5, 0xfd);

// 灰阶层次
pub const WINDOW_BG: Color32 = Color32::from_rgb(0xf3, 0xf5, 0xf9);
pub const PANEL_BG: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
pub const PANEL_SOFT: Color32 = Color32::from_rgb(0xf8, 0xfa, 0xfd);
pub const SECTION_BG: Color32 = Color32::from_rgb(0xf5, 0xf7, 0xfb);
pub const CARD_BG: Color32 = Color32::from_rgb(0xfb, 0xfc, 0xfe);
pub const INPUT_BG: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);

pub const BORDER: Color32 = Color32::from_rgb(0xde, 0xe3, 0xea);
pub const BORDER_STRONG: Color32 = Color32::from_rgb(0xc9, 0xd2, 0xdd);

pub const TEXT: Color32 = Color32::from_rgb(0x1f, 0x24, 0x2e);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x6b, 0x74, 0x80);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(0x9a, 0xa3, 0xad);

// 语义色
pub const OK_GREEN: Color32 = Color32::from_rgb(0x2f, 0x9e, 0x5f);
pub const OK_GREEN_SOFT: Color32 = Color32::from_rgb(0xe6, 0xf5, 0xec);
pub const WARN: Color32 = Color32::from_rgb(0xdd, 0x8a, 0x22);
pub const DANGER: Color32 = Color32::from_rgb(0xd9, 0x4b, 0x3d);
pub const DANGER_SOFT: Color32 = Color32::from_rgb(0xfb, 0xea, 0xe7);

// 图标色
pub const FOLDER: Color32 = Color32::from_rgb(0xf5, 0xb8, 0x3d);
pub const FOLDER_DARK: Color32 = Color32::from_rgb(0xd8, 0x9a, 0x1c);
pub const ARCHIVE: Color32 = Color32::from_rgb(0x8a, 0xa8, 0xe0);
pub const ARCHIVE_DARK: Color32 = Color32::from_rgb(0x5c, 0x7a, 0xc2);
pub const LOCK: Color32 = Color32::from_rgb(0xc9, 0x5f, 0x3c);
pub const LOCK_SOFT: Color32 = Color32::from_rgb(0xfb, 0xec, 0xe4);

// 选择 / 悬停
pub const SELECT_BG: Color32 = Color32::from_rgb(0xe3, 0xec, 0xfb);
pub const HOVER_BG: Color32 = Color32::from_rgb(0xf0, 0xf4, 0xfa);

fn install_style(ctx: &egui::Context) {
    let mut style = (*ctx.global_style()).clone();
    let mut v = egui::Visuals::light();

    // ---- 面板与窗口
    v.panel_fill = WINDOW_BG;
    v.window_fill = PANEL_BG;
    v.extreme_bg_color = PANEL_BG;
    v.faint_bg_color = PANEL_SOFT;
    v.override_text_color = Some(TEXT);
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_corner_radius = CornerRadius::same(10);
    v.window_shadow = Shadow {
        offset: [0, 6],
        blur: 24,
        spread: 0,
        color: Color32::from_rgba_unmultiplied(0x1f, 0x24, 0x2e, 48),
    };

    // ---- 选区
    v.selection.bg_fill = SELECT_BG;
    v.selection.stroke = Stroke::new(1.0, ACCENT);

    // ---- 超链接
    v.hyperlink_color = ACCENT;

    // ---- 控件圆角
    let radius = CornerRadius::same(6);
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
        &mut v.widgets.noninteractive,
    ] {
        w.corner_radius = radius;
    }

    // 默认按钮
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0xf2, 0xf4, 0xf8);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.hovered.weak_bg_fill = HOVER_BG;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, ACCENT_DARK);
    v.widgets.active.weak_bg_fill = ACCENT_SOFT;
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.active.fg_stroke = Stroke::new(1.0, ACCENT_DARK);
    v.widgets.open.weak_bg_fill = ACCENT_SOFTER;
    v.widgets.open.bg_stroke = Stroke::new(1.0, ACCENT);

    // 非交互元素（标签等）
    v.widgets.noninteractive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.noninteractive.bg_stroke = Stroke::NONE;

    // ---- 间距
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.window_margin = egui::Margin::same(14);
    style.spacing.interact_size = egui::vec2(0.0, 26.0);

    // ---- 字体
    use egui::FontFamily::Proportional;
    use egui::FontId;
    use egui::TextStyle::*;
    style.text_styles = [
        (Small, FontId::new(11.0, Proportional)),
        (Body, FontId::new(13.5, Proportional)),
        (Button, FontId::new(13.5, Proportional)),
        (Heading, FontId::new(18.0, Proportional)),
        (Monospace, FontId::new(12.5, egui::FontFamily::Monospace)),
    ]
    .into();

    style.visuals = v;

    ctx.set_global_style(Arc::new(style));
}