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
    /// 不弹对话框，直接压缩为同目录同名 .zip。
    CompressHere(Vec<PathBuf>),
    /// 直接解压到压缩包所在目录。
    ExtractHere(PathBuf),
    /// 打开解压对话框。
    ExtractTo(PathBuf),
    /// 在指定目录启动。
    Browse(PathBuf),
    RegisterShell,
    UnregisterShell,
}

/// 只保留真实存在的路径。
///
/// 右键菜单用 `%*` 传多选文件，万一 shell 没有展开（旧系统或 Player 多选模型
/// 不生效），这里会得到字面量 `%*`；过滤掉之后 `parse` 会退回普通启动，
/// 而不是去压缩一个名为 `%*` 的文件。
fn existing_paths(args: &[String]) -> Vec<PathBuf> {
    args.iter()
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .collect()
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
            // 即使过滤后为空也返回该变体，交由 main 的空路径分支优雅退出，
            // 避免 fall-through 到 Normal 而启动 GUI（右键传入 %* 未展开时尤其重要）。
            return Startup::Compress(existing_paths(&args[1..]));
        }
        "--compress-here" => {
            return Startup::CompressHere(existing_paths(&args[1..]));
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
