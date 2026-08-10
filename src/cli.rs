//! 命令行入口解析，供右键菜单调用。

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum Startup {
    /// 普通启动，浏览本地目录。
    Normal,
    /// 打开一个压缩包。
    Open(PathBuf),
    /// 打开压缩对话框，预填这些源文件。
    Compress(Vec<PathBuf>),
    /// 直接解压到压缩包所在目录。
    ExtractHere(PathBuf),
    /// 打开解压对话框。
    ExtractTo(PathBuf),
    /// 在指定目录启动。
    Browse(PathBuf),
    RegisterShell,
    UnregisterShell,
}

pub fn parse() -> Startup {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        return Startup::Normal;
    }

    match args[0].as_str() {
        "--register-shell" => return Startup::RegisterShell,
        "--unregister-shell" => return Startup::UnregisterShell,
        "--compress" => {
            let paths: Vec<PathBuf> = args[1..].iter().map(PathBuf::from).collect();
            if !paths.is_empty() {
                return Startup::Compress(paths);
            }
        }
        "--extract-here" => {
            if let Some(p) = args.get(1) {
                return Startup::ExtractHere(PathBuf::from(p));
            }
        }
        "--extract-to" => {
            if let Some(p) = args.get(1) {
                return Startup::ExtractTo(PathBuf::from(p));
            }
        }
        other if !other.starts_with("--") => {
            let p = PathBuf::from(other);
            if p.is_dir() {
                return Startup::Browse(p);
            }
            if p.is_file() {
                return Startup::Open(p);
            }
        }
        _ => {}
    }
    Startup::Normal
}
