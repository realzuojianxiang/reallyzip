//! 命令行入口解析，供右键菜单调用。

use std::path::PathBuf;

use crate::diag;

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

/// 重建被空格拆散的路径。
///
/// 右键菜单命令里的 `%*` / `%1` 未加引号时，路径含空格会被 Windows 按空格拆成
/// 多个参数（如 `C:\我的文档\报告.txt` → `C:\我的文档\报告.txt` 被拆成三段）。
/// 这里把相邻 token 贪心拼接成「真实存在的最长路径」，从而还原出完整路径。
/// 对本来就正确的带引号参数无副作用（单个 token 已存在时直接采纳）。
fn reconstruct_paths(tokens: &[String]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let mut merged: Option<String> = None;
        let mut end = i + 1;
        // 从最长拼接开始尝试，命中真实存在的路径即采纳
        for j in (i + 1..=tokens.len()).rev() {
            let cand = tokens[i..j].join(" ");
            if std::path::Path::new(&cand).exists() {
                merged = Some(cand);
                end = j;
                break;
            }
        }
        match merged {
            Some(p) => {
                out.push(PathBuf::from(p));
                i = end;
            }
            None => {
                // 没有任何拼接存在于磁盘：原样保留，后续交给 .exists() 过滤
                out.push(PathBuf::from(&tokens[i]));
                i += 1;
            }
        }
    }
    out
}

/// 只保留真实存在的路径。
///
/// 右键菜单用 `%*` 传多选文件，且 `%*` 不会被 Windows 加引号；空间含空格的路径
/// 会被拆散，`reconstruct_paths` 先尝试还原完整路径，再按存在性过滤。
/// 万一 shell 没有展开（旧系统或 Player 多选模型不生效）而得到字面量 `%*`，
/// 过滤后为空，`parse` 会退回普通启动，而不是去压缩一个名为 `%*` 的文件。
fn existing_paths(args: &[String]) -> Vec<PathBuf> {
    let reconstructed = reconstruct_paths(args);
    diag::log(&format!(
        "parse: reconstruct_paths({args:?}) = {reconstructed:?}"
    ));
    let mut kept: Vec<PathBuf> = reconstructed.into_iter().filter(|p| p.exists()).collect();
    // 注册命令用 `%1 %*`：单选时 `%1` 与 `%*` 的首项重复，去重避免同一文件进压缩包两次。
    kept.sort();
    kept.dedup();
    diag::log(&format!("parse: existing_paths after filter+dedup = {kept:?}"));
    kept
}

pub fn parse() -> Startup {
    let args: Vec<String> = std::env::args().skip(1).collect();
    diag::log(&format!("parse: raw tokens = {args:?}"));
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
            if let Some(p) = reconstruct_paths(&args[1..]).into_iter().next() {
                return Startup::ExtractHere(p);
            }
        }
        "--extract-to" => {
            if let Some(p) = reconstruct_paths(&args[1..]).into_iter().next() {
                return Startup::ExtractTo(p);
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
