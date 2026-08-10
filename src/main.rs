//! ReallyZip —— 用 Rust 编写的图形化压缩/解压工具（参考 WinRAR 的体验）。
//!
//! 入口：解析命令行、装配模块、安装中文字体与主题、启动 eframe 主窗口。

// Windows 下发布版作为 GUI 程序运行，不弹出终端控制台窗口。
#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod app;
mod archive;
mod cli;
mod shell;
mod task;
mod ui;
mod util;
mod volume;

#[cfg(test)]
mod tests;

use eframe::NativeOptions;
use egui::ViewportBuilder;

fn main() -> eframe::Result {
    let startup = cli::parse();

    let options = NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("ReallyZip")
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([680.0, 420.0])
            .with_icon(load_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "ReallyZip",
        options,
        Box::new(|cc| {
            crate::ui::theme::install(&cc.egui_ctx);
            Ok(Box::new(app::ReallyZipApp::new(startup)))
        }),
    )
}

/// 加载嵌入的窗口图标（一个简易的压缩包图标）。
fn load_icon() -> egui::IconData {
    // 16x16 的蓝色存档图标，避免依赖任何图片资源文件。
    const W: u32 = 16;
    const H: u32 = 16;
    let mut rgba = Vec::with_capacity((W * H) as usize * 4);
    for y in 0..H {
        for x in 0..W {
            // 圆角矩形主体
            let margin = 2u32;
            let in_body = x >= margin
                && x < W - margin
                && y >= margin
                && y < H - margin;
            // 绑带高光
            let band = (x == 5 || x == 6) && y >= margin && y < H - margin;
            let (r, g, b, a) = if in_body {
                if band {
                    (230, 238, 250, 255)
                } else {
                    (78, 107, 168, 255)
                }
            } else {
                (0, 0, 0, 0)
            };
            rgba.extend_from_slice(&[r, g, b, a]);
        }
    }
    egui::IconData {
        rgba,
        width: W,
        height: H,
    }
}
