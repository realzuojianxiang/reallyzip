//! 分卷压缩包的切分与合并。
//!
//! 采用与 7-Zip 一致的裸切分方案：`archive.zip` 被切成
//! `archive.zip.001`、`archive.zip.002` …… 顺序拼接即可还原原始压缩包。

use anyhow::{Context, Result, bail};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use crate::task::Reporter;

const BUF: usize = 256 * 1024;

/// 常用分卷尺寸预设（字节，0 表示不分卷）。
pub const PRESETS: &[(&str, u64)] = &[
    ("不分卷", 0),
    ("1.44 MB (软盘)", 1_457_664),
    ("10 MB", 10 * 1024 * 1024),
    ("100 MB", 100 * 1024 * 1024),
    ("700 MB (CD)", 700 * 1024 * 1024),
    ("4095 MB (FAT32)", 4095 * 1024 * 1024),
];

/// 判断是否是形如 `xxx.001` 的分卷文件。
pub fn is_volume_part(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.len() == 3 && e.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or(false)
}

/// 由任意一个分卷推出基础名：`a.zip.003` -> `a.zip`
fn volume_base(path: &Path) -> Option<PathBuf> {
    if !is_volume_part(path) {
        return None;
    }
    let s = path.to_string_lossy();
    let cut = s.len().checked_sub(4)?; // 去掉 ".NNN"
    Some(PathBuf::from(&s[..cut]))
}

/// 收集某个分卷所属的完整分卷序列（从 001 开始，遇到缺号即停止）。
pub fn collect_volumes(any_part: &Path) -> Result<Vec<PathBuf>> {
    let base = volume_base(any_part).context("不是有效的分卷文件名")?;
    let mut out = Vec::new();
    for i in 1..=999u32 {
        let p = PathBuf::from(format!("{}.{:03}", base.to_string_lossy(), i));
        if p.exists() {
            out.push(p);
        } else {
            break;
        }
    }
    if out.is_empty() {
        bail!("找不到分卷 {}.001", base.display());
    }
    Ok(out)
}

/// 把分卷合并成一个临时文件，返回该临时文件路径。
pub fn merge_volumes(volumes: &[PathBuf], rep: &Reporter) -> Result<PathBuf> {
    let total: u64 = volumes
        .iter()
        .filter_map(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .sum();
    rep.total(total);

    let stem = volumes[0]
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "archive.zip".into());
    let tmp_dir = std::env::temp_dir().join("rustrar");
    std::fs::create_dir_all(&tmp_dir).ok();
    let out_path = tmp_dir.join(format!(
        "merged_{}_{}",
        std::process::id(),
        sanitize(&stem)
    ));

    let mut out = BufWriter::with_capacity(BUF, File::create(&out_path)?);
    let mut buf = vec![0u8; BUF];
    let mut done: u64 = 0;

    for (i, v) in volumes.iter().enumerate() {
        rep.progress(done, &format!("合并分卷 {}/{}", i + 1, volumes.len()));
        let mut f = BufReader::with_capacity(BUF, File::open(v)?);
        loop {
            rep.check_cancel()?;
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])?;
            done += n as u64;
            rep.progress(done, &format!("合并分卷 {}/{}", i + 1, volumes.len()));
        }
    }
    out.flush()?;
    Ok(out_path)
}

/// 把一个完整文件切分成多个分卷，返回生成的分卷路径列表。
///
/// `dest_base` 例如 `D:\out\pack.zip`，生成 `pack.zip.001`、`pack.zip.002`……
pub fn split_file(src: &Path, dest_base: &Path, part_size: u64, rep: &Reporter) -> Result<Vec<PathBuf>> {
    if part_size == 0 {
        bail!("分卷大小必须大于 0");
    }
    let total = std::fs::metadata(src)?.len();
    rep.total(total);

    let mut input = BufReader::with_capacity(BUF, File::open(src)?);
    let mut buf = vec![0u8; BUF];
    let mut parts: Vec<PathBuf> = Vec::new();
    let mut done: u64 = 0;
    let mut index = 1u32;
    let mut eof = false;

    while !eof {
        let part_path = PathBuf::from(format!("{}.{:03}", dest_base.to_string_lossy(), index));
        let mut out = BufWriter::with_capacity(BUF, File::create(&part_path)?);
        let mut written: u64 = 0;

        while written < part_size {
            rep.check_cancel()?;
            let want = std::cmp::min(BUF as u64, part_size - written) as usize;
            let n = input.read(&mut buf[..want])?;
            if n == 0 {
                eof = true;
                break;
            }
            out.write_all(&buf[..n])?;
            written += n as u64;
            done += n as u64;
            rep.progress(done, &format!("正在写入分卷 {index:03}"));
        }
        out.flush()?;
        drop(out);

        if written == 0 {
            // 刚好在分卷边界结束，删掉空分卷
            let _ = std::fs::remove_file(&part_path);
            break;
        }
        parts.push(part_path);
        index += 1;
        if index > 999 {
            bail!("分卷数量超过 999，请增大分卷体积");
        }
    }

    if parts.is_empty() {
        bail!("没有生成任何分卷");
    }
    Ok(parts)
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c })
        .collect()
}
