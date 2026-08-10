//! 通用工具函数：大小格式化、时间格式化、文件类型识别等。

use chrono::{DateTime, Local, NaiveDateTime};
use std::path::Path;
use std::time::SystemTime;

/// 人类可读的字节大小。
pub fn format_size(bytes: u64) -> String {
    const K: f64 = 1024.0;
    let b = bytes as f64;
    if bytes < 1024 {
        format!("{bytes} B")
    } else if b < K * K {
        format!("{:.1} KB", b / K)
    } else if b < K * K * K {
        format!("{:.1} MB", b / (K * K))
    } else {
        format!("{:.2} GB", b / (K * K * K))
    }
}

/// 千分位分隔的整数，用于状态栏统计。
pub fn format_thousands(n: u64) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*c as char);
    }
    out
}

/// 压缩率百分比字符串。
pub fn ratio_str(original: u64, packed: u64) -> String {
    if original == 0 {
        "-".to_string()
    } else {
        format!("{}%", ((packed as f64 / original as f64) * 100.0).round() as i64)
    }
}

pub fn system_time_to_naive(t: SystemTime) -> Option<NaiveDateTime> {
    let dt: DateTime<Local> = t.into();
    Some(dt.naive_local())
}

pub fn fmt_time(t: Option<NaiveDateTime>) -> String {
    match t {
        Some(t) => t.format("%Y-%m-%d %H:%M").to_string(),
        None => "-".to_string(),
    }
}

/// 取小写扩展名（不含点）。
pub fn ext_lower(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((head, ext)) if !head.is_empty() && !ext.is_empty() && !ext.contains(['/', '\\']) => {
            ext.to_ascii_lowercase()
        }
        _ => String::new(),
    }
}

/// 中文的文件类型描述，模仿资源管理器的“类型”列。
pub fn file_kind(name: &str, is_dir: bool) -> String {
    if is_dir {
        return "文件夹".to_string();
    }
    let ext = ext_lower(name);
    let desc = match ext.as_str() {
        "" => return "文件".to_string(),
        "zip" | "7z" | "rar" | "gz" | "bz2" | "xz" | "zst" | "tar" | "cab" => "压缩文件",
        "txt" | "log" | "ini" | "cfg" | "conf" | "csv" => "文本文档",
        "md" => "Markdown 文档",
        "json" | "xml" | "yaml" | "yml" | "toml" => "配置/数据文件",
        "rs" | "c" | "h" | "cpp" | "hpp" | "py" | "js" | "ts" | "go" | "java" | "cs" | "sh"
        | "html" | "css" => "源代码文件",
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "ico" | "svg" | "tif" | "tiff" => "图片文件",
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" => "音频文件",
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" => "视频文件",
        "pdf" => "PDF 文档",
        "doc" | "docx" => "Word 文档",
        "xls" | "xlsx" => "Excel 工作表",
        "ppt" | "pptx" => "PowerPoint 演示文稿",
        "exe" | "msi" => "应用程序",
        "dll" | "so" | "dylib" => "动态链接库",
        "ttf" | "otf" | "ttc" => "字体文件",
        _ => return format!("{} 文件", ext.to_uppercase()),
    };
    desc.to_string()
}

/// 是否是本程序能直接打开的压缩包。
pub fn is_archive_path(p: &Path) -> bool {
    let name = p
        .file_name()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if name.ends_with(".zip") {
        return true;
    }
    // 分卷：xxx.zip.001
    if let Some((head, tail)) = name.rsplit_once('.')
        && tail.len() == 3
        && tail.chars().all(|c| c.is_ascii_digit())
        && head.ends_with(".zip")
    {
        return true;
    }
    false
}

/// 把任意路径分隔符统一成 zip 内部使用的 `/`。
pub fn normalize_zip_path(s: &str) -> String {
    s.replace('\\', "/").trim_start_matches('/').to_string()
}

/// 取 zip 内部路径的最后一段。
pub fn base_name(path: &str) -> &str {
    let p = path.trim_end_matches('/');
    match p.rsplit_once('/') {
        Some((_, n)) => n,
        None => p,
    }
}

/// 取 zip 内部路径的父目录（不含尾部 `/`）。
pub fn parent_dir(path: &str) -> String {
    let p = path.trim_end_matches('/');
    match p.rsplit_once('/') {
        Some((head, _)) => head.to_string(),
        None => String::new(),
    }
}

/// 生成不冲突的文件名：a.txt -> a (1).txt
pub fn unique_path(path: &Path) -> std::path::PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let parent = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());
    let ext = path.extension().map(|s| s.to_string_lossy().to_string());
    for i in 1..10_000u32 {
        let name = match &ext {
            Some(e) => format!("{stem} ({i}).{e}"),
            None => format!("{stem} ({i})"),
        };
        let candidate = parent.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    path.to_path_buf()
}

/// 调用系统默认程序打开文件或目录。
pub fn open_with_system(path: &Path) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(path.as_os_str())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(path).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
}

/// 在资源管理器中定位文件。
pub fn reveal_in_explorer(path: &Path) {
    #[cfg(windows)]
    {
        // 路径含空格时必须加引号，否则 explorer 会被空格截断参数。
        let _ = std::process::Command::new("explorer")
            .arg(format!("/select,\"{}\"", path.display()))
            .spawn();
    }
    #[cfg(not(windows))]
    {
        if let Some(dir) = path.parent() {
            open_with_system(dir);
        }
    }
}
