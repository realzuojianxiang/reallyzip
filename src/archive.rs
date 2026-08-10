//! ZIP 引擎：列出条目、创建、追加、删除、解压、测试。

use anyhow::{Context, Result, bail};
use chrono::NaiveDateTime;
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::result::ZipError;
use zip::write::SimpleFileOptions;
use zip::{AesMode, CompressionMethod, ZipArchive, ZipWriter};

use crate::task::Reporter;
use crate::util;
use crate::volume;

const BUF: usize = 128 * 1024;

// ---------------------------------------------------------------- 数据模型

#[derive(Clone, Debug)]
pub struct ArchiveEntry {
    pub index: usize,
    /// zip 内部路径，目录不带尾部斜杠。
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub compressed: u64,
    pub crc32: u32,
    pub modified: Option<NaiveDateTime>,
    pub encrypted: bool,
    pub method: String,
}

/// 已打开的压缩包。
pub struct OpenArchive {
    /// 用户看到的路径（分卷时是 .001）。
    pub display_path: PathBuf,
    /// 实际用于读取的文件（分卷时是合并后的临时文件）。
    pub real_path: PathBuf,
    pub entries: Vec<ArchiveEntry>,
    pub volumes: Vec<PathBuf>,
    pub temp_merged: bool,
    pub total_size: u64,
    pub total_packed: u64,
    pub has_encrypted: bool,
}

impl OpenArchive {
    pub fn name(&self) -> String {
        self.display_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| self.display_path.to_string_lossy().to_string())
    }

    pub fn file_count(&self) -> usize {
        self.entries.iter().filter(|e| !e.is_dir).count()
    }
}

impl Drop for OpenArchive {
    fn drop(&mut self) {
        if self.temp_merged {
            let _ = fs::remove_file(&self.real_path);
        }
    }
}

/// 压缩包内某一层目录下的一个可见项。
#[derive(Clone, Debug)]
pub struct Node {
    pub name: String,
    pub full: String,
    pub is_dir: bool,
    pub size: u64,
    pub compressed: u64,
    pub crc32: Option<u32>,
    pub modified: Option<NaiveDateTime>,
    pub encrypted: bool,
    #[allow(dead_code)]
    pub method: String,
}

/// 压缩级别。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    Store,
    Fastest,
    Fast,
    Normal,
    Good,
    Best,
}

impl Level {
    pub const ALL: [Level; 6] = [
        Level::Store,
        Level::Fastest,
        Level::Fast,
        Level::Normal,
        Level::Good,
        Level::Best,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Level::Store => "存储（不压缩）",
            Level::Fastest => "最快",
            Level::Fast => "较快",
            Level::Normal => "标准",
            Level::Good => "较好",
            Level::Best => "最大压缩",
        }
    }

    fn method(self) -> CompressionMethod {
        match self {
            Level::Store => CompressionMethod::Stored,
            _ => CompressionMethod::Deflated,
        }
    }

    fn level(self) -> Option<i64> {
        match self {
            Level::Store => None,
            Level::Fastest => Some(1),
            Level::Fast => Some(3),
            Level::Normal => Some(6),
            Level::Good => Some(8),
            Level::Best => Some(9),
        }
    }
}

#[derive(Clone, Debug)]
pub struct CreateOptions {
    pub level: Level,
    pub password: Option<String>,
    pub split_size: u64,
}

impl Default for CreateOptions {
    fn default() -> Self {
        Self {
            level: Level::Normal,
            password: None,
            split_size: 0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Overwrite {
    Always,
    Skip,
    AutoRename,
}

impl Overwrite {
    pub fn label(self) -> &'static str {
        match self {
            Overwrite::Always => "覆盖同名文件",
            Overwrite::Skip => "跳过同名文件",
            Overwrite::AutoRename => "自动重命名",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExtractOptions {
    pub dest: PathBuf,
    /// None 表示全部解压；否则只解压这些 zip 内部路径（含其子项）。
    pub selection: Option<HashSet<String>>,
    pub password: Option<String>,
    pub keep_paths: bool,
    pub overwrite: Overwrite,
}

// ---------------------------------------------------------------- 打开与列举

fn method_name(m: CompressionMethod) -> String {
    match m {
        CompressionMethod::Stored => "存储".to_string(),
        CompressionMethod::Deflated => "Deflate".to_string(),
        other => format!("{other:?}"),
    }
}

fn zip_dt_to_naive(dt: Option<zip::DateTime>) -> Option<NaiveDateTime> {
    dt.and_then(|d| NaiveDateTime::try_from(d).ok())
}

/// 读取压缩包的全部条目元数据（不需要密码）。
pub fn read_entries(path: &Path) -> Result<Vec<ArchiveEntry>> {
    let file = File::open(path).with_context(|| format!("无法打开 {}", path.display()))?;
    let mut za = ZipArchive::new(BufReader::with_capacity(BUF, file))
        .with_context(|| format!("不是有效的 ZIP 文件：{}", path.display()))?;

    let mut out = Vec::with_capacity(za.len());
    for i in 0..za.len() {
        let f = za.by_index_raw(i)?;
        let raw = f.name().to_string();
        let is_dir = f.is_dir();
        let norm = util::normalize_zip_path(&raw);
        let norm = norm.trim_end_matches('/').to_string();
        if norm.is_empty() {
            continue;
        }
        out.push(ArchiveEntry {
            index: i,
            path: norm,
            is_dir,
            size: f.size(),
            compressed: f.compressed_size(),
            crc32: f.crc32(),
            modified: zip_dt_to_naive(f.last_modified()),
            encrypted: f.encrypted(),
            method: method_name(f.compression()),
        });
    }
    Ok(out)
}

/// 打开压缩包（自动识别并合并分卷）。
pub fn open(path: &Path, rep: &Reporter) -> Result<OpenArchive> {
    let (real_path, volumes, temp_merged) = if volume::is_volume_part(path) {
        let vols = volume::collect_volumes(path)?;
        rep.log(format!("检测到 {} 个分卷，正在合并…", vols.len()));
        let merged = volume::merge_volumes(&vols, rep)?;
        (merged, vols, true)
    } else {
        (path.to_path_buf(), Vec::new(), false)
    };

    let entries = match read_entries(&real_path) {
        Ok(e) => e,
        Err(err) => {
            if temp_merged {
                let _ = fs::remove_file(&real_path);
            }
            return Err(err);
        }
    };

    let total_size = entries.iter().filter(|e| !e.is_dir).map(|e| e.size).sum();
    let total_packed = entries
        .iter()
        .filter(|e| !e.is_dir)
        .map(|e| e.compressed)
        .sum();
    let has_encrypted = entries.iter().any(|e| e.encrypted);

    Ok(OpenArchive {
        display_path: path.to_path_buf(),
        real_path,
        entries,
        volumes,
        temp_merged,
        total_size,
        total_packed,
        has_encrypted,
    })
}

/// 计算压缩包内某一层目录的可见子项（自动补齐隐式目录）。
pub fn children_of(entries: &[ArchiveEntry], dir: &str) -> Vec<Node> {
    let prefix = if dir.is_empty() {
        String::new()
    } else {
        format!("{dir}/")
    };

    let mut dirs: BTreeMap<String, Node> = BTreeMap::new();
    let mut files: BTreeMap<String, Node> = BTreeMap::new();

    for e in entries {
        if !prefix.is_empty() && !e.path.starts_with(&prefix) {
            continue;
        }
        let rest = &e.path[prefix.len()..];
        if rest.is_empty() {
            continue;
        }
        match rest.split_once('/') {
            Some((head, _)) => {
                // 深层条目 -> 归并到直接子目录
                let full = format!("{prefix}{head}");
                let node = dirs.entry(head.to_string()).or_insert_with(|| Node {
                    name: head.to_string(),
                    full,
                    is_dir: true,
                    size: 0,
                    compressed: 0,
                    crc32: None,
                    modified: None,
                    encrypted: false,
                    method: "文件夹".to_string(),
                });
                if !e.is_dir {
                    node.size += e.size;
                    node.compressed += e.compressed;
                }
                node.encrypted |= e.encrypted;
                if node.modified.is_none() || (e.modified.is_some() && e.modified > node.modified) {
                    node.modified = e.modified;
                }
            }
            None => {
                if e.is_dir {
                    let node = dirs.entry(rest.to_string()).or_insert_with(|| Node {
                        name: rest.to_string(),
                        full: e.path.clone(),
                        is_dir: true,
                        size: 0,
                        compressed: 0,
                        crc32: None,
                        modified: e.modified,
                        encrypted: false,
                        method: "文件夹".to_string(),
                    });
                    if node.modified.is_none() {
                        node.modified = e.modified;
                    }
                } else {
                    files.insert(
                        rest.to_string(),
                        Node {
                            name: rest.to_string(),
                            full: e.path.clone(),
                            is_dir: false,
                            size: e.size,
                            compressed: e.compressed,
                            crc32: Some(e.crc32),
                            modified: e.modified,
                            encrypted: e.encrypted,
                            method: e.method.clone(),
                        },
                    );
                }
            }
        }
    }

    let mut out: Vec<Node> = dirs.into_values().collect();
    out.extend(files.into_values());
    out
}

// ---------------------------------------------------------------- 创建

struct Item {
    abs: PathBuf,
    rel: String,
    is_dir: bool,
    size: u64,
}

fn collect_items(sources: &[PathBuf]) -> Result<(Vec<Item>, u64)> {
    let mut items = Vec::new();
    let mut total = 0u64;

    for src in sources {
        let md = fs::metadata(src).with_context(|| format!("无法读取 {}", src.display()))?;
        let name = src
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| src.to_string_lossy().replace([':', '\\', '/'], "_"));

        if md.is_dir() {
            items.push(Item {
                abs: src.clone(),
                rel: name.clone(),
                is_dir: true,
                size: 0,
            });
            for entry in WalkDir::new(src).min_depth(1).follow_links(false) {
                let entry = entry?;
                let rel_inner = entry
                    .path()
                    .strip_prefix(src)?
                    .to_string_lossy()
                    .replace('\\', "/");
                let rel = format!("{name}/{rel_inner}");
                if entry.file_type().is_dir() {
                    items.push(Item {
                        abs: entry.path().to_path_buf(),
                        rel,
                        is_dir: true,
                        size: 0,
                    });
                } else if entry.file_type().is_file() {
                    let size = entry.metadata()?.len();
                    total += size;
                    items.push(Item {
                        abs: entry.path().to_path_buf(),
                        rel,
                        is_dir: false,
                        size,
                    });
                }
            }
        } else if md.is_file() {
            total += md.len();
            items.push(Item {
                abs: src.clone(),
                rel: name,
                is_dir: false,
                size: md.len(),
            });
        }
    }

    if items.is_empty() {
        bail!("没有可压缩的文件");
    }
    Ok((items, total))
}

fn file_mtime(p: &Path) -> Option<NaiveDateTime> {
    fs::metadata(p)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(util::system_time_to_naive)
}

fn write_items<W: Write + std::io::Seek>(
    zw: &mut ZipWriter<W>,
    items: &[Item],
    opt: &CreateOptions,
    total: u64,
    rep: &Reporter,
    skip_names: Option<&HashSet<String>>,
    skipped: &mut Vec<String>,
) -> Result<u64> {
    let mut buf = vec![0u8; BUF];
    let mut done = 0u64;
    rep.total(total);

    for item in items {
        rep.check_cancel()?;
        if let Some(existing) = skip_names
            && existing.contains(&item.rel)
        {
            skipped.push(item.rel.clone());
            continue;
        }

        let mtime = file_mtime(&item.abs);
        let mut base = SimpleFileOptions::default()
            .compression_method(opt.level.method())
            .compression_level(opt.level.level())
            .large_file(item.size >= u32::MAX as u64);
        if let Some(dt) = mtime.and_then(|m| zip::DateTime::try_from(m).ok()) {
            base = base.last_modified_time(dt);
        }

        if item.is_dir {
            match opt.password.as_deref() {
                Some(pw) if !pw.is_empty() => {
                    zw.add_directory(&item.rel, base.with_aes_encryption(AesMode::Aes256, pw))?
                }
                _ => zw.add_directory(&item.rel, base)?,
            }
            continue;
        }

        rep.progress(done, &item.rel);
        match opt.password.as_deref() {
            Some(pw) if !pw.is_empty() => {
                zw.start_file(&item.rel, base.with_aes_encryption(AesMode::Aes256, pw))?
            }
            _ => zw.start_file(&item.rel, base)?,
        }

        let mut f = BufReader::with_capacity(
            BUF,
            File::open(&item.abs).with_context(|| format!("无法读取 {}", item.abs.display()))?,
        );
        loop {
            rep.check_cancel()?;
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            zw.write_all(&buf[..n])?;
            done += n as u64;
            rep.progress(done, &item.rel);
        }
    }
    Ok(done)
}

/// 创建新的压缩包（可加密、可分卷）。
pub fn create(
    sources: &[PathBuf],
    dest: &Path,
    opt: &CreateOptions,
    rep: &Reporter,
) -> Result<String> {
    let (items, total) = collect_items(sources)?;
    let file_count = items.iter().filter(|i| !i.is_dir).count();

    if let Some(parent) = dest.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    // 分卷时先写到临时文件，最后再切分
    let building = if opt.split_size > 0 {
        let tmp_dir = std::env::temp_dir().join("rustrar");
        fs::create_dir_all(&tmp_dir)?;
        tmp_dir.join(format!("building_{}.zip", std::process::id()))
    } else {
        dest.to_path_buf()
    };

    let result = (|| -> Result<()> {
        let out = File::create(&building)
            .with_context(|| format!("无法创建 {}", building.display()))?;
        let mut zw = ZipWriter::new(BufWriter::with_capacity(BUF, out));
        let mut skipped = Vec::new();
        write_items(&mut zw, &items, opt, total, rep, None, &mut skipped)?;
        let mut inner = zw.finish()?;
        inner.flush()?;
        Ok(())
    })();

    if let Err(e) = result {
        let _ = fs::remove_file(&building);
        return Err(e);
    }

    if opt.split_size > 0 {
        rep.log("正在切分为分卷…");
        let parts = match volume::split_file(&building, dest, opt.split_size, rep) {
            Ok(p) => p,
            Err(e) => {
                let _ = fs::remove_file(&building);
                return Err(e);
            }
        };
        let _ = fs::remove_file(&building);
        let packed: u64 = parts
            .iter()
            .filter_map(|p| fs::metadata(p).ok())
            .map(|m| m.len())
            .sum();
        return Ok(format!(
            "已创建 {} 个分卷，共 {} 个文件，原始 {} → 压缩后 {}（{}）",
            parts.len(),
            file_count,
            util::format_size(total),
            util::format_size(packed),
            util::ratio_str(total, packed)
        ));
    }

    let packed = fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
    Ok(format!(
        "已压缩 {} 个文件：{} → {}（{}）",
        file_count,
        util::format_size(total),
        util::format_size(packed),
        util::ratio_str(total, packed)
    ))
}

/// 向已有压缩包追加文件。
pub fn append(
    archive: &Path,
    sources: &[PathBuf],
    opt: &CreateOptions,
    rep: &Reporter,
) -> Result<String> {
    let existing: HashSet<String> = read_entries(archive)?
        .into_iter()
        .map(|e| e.path)
        .collect();
    let (items, total) = collect_items(sources)?;

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(archive)
        .with_context(|| format!("无法写入 {}", archive.display()))?;
    let mut zw = ZipWriter::new_append(file)?;
    let mut skipped = Vec::new();
    let opt2 = CreateOptions {
        split_size: 0,
        ..opt.clone()
    };
    write_items(
        &mut zw,
        &items,
        &opt2,
        total,
        rep,
        Some(&existing),
        &mut skipped,
    )?;
    let mut inner = zw.finish()?;
    inner.flush()?;

    let added = items.iter().filter(|i| !i.is_dir).count() - skipped.len();
    if skipped.is_empty() {
        Ok(format!("已向压缩包追加 {added} 个文件"))
    } else {
        Ok(format!(
            "已追加 {} 个文件，跳过 {} 个同名条目",
            added,
            skipped.len()
        ))
    }
}

/// 从压缩包中删除条目（无损重写，不重新压缩）。
pub fn delete_entries(archive: &Path, targets: &HashSet<String>, rep: &Reporter) -> Result<String> {
    let src = File::open(archive)?;
    let mut za = ZipArchive::new(BufReader::with_capacity(BUF, src))?;
    let n = za.len();
    rep.total(n as u64);

    let tmp = archive.with_extension("rustrar_tmp");
    let out = File::create(&tmp)?;
    let mut zw = ZipWriter::new(BufWriter::with_capacity(BUF, out));

    let mut removed = 0usize;
    let mut kept = 0usize;

    let result = (|| -> Result<()> {
        for i in 0..n {
            rep.check_cancel()?;
            rep.progress(i as u64, "正在重写压缩包…");
            let f = za.by_index_raw(i)?;
            let name = util::normalize_zip_path(f.name());
            let trimmed = name.trim_end_matches('/').to_string();
            let hit = targets.iter().any(|t| {
                trimmed == *t || trimmed.starts_with(&format!("{t}/"))
            });
            if hit {
                removed += 1;
                continue;
            }
            kept += 1;
            zw.raw_copy_file(f)?;
        }
        let mut inner = zw.finish()?;
        inner.flush()?;
        Ok(())
    })();

    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }

    drop(za);
    fs::remove_file(archive)?;
    fs::rename(&tmp, archive)?;
    Ok(format!("已删除 {removed} 个条目，保留 {kept} 个"))
}

// ---------------------------------------------------------------- 解压

fn safe_out_path(dest: &Path, raw_name: &str, keep_paths: bool) -> Option<PathBuf> {
    let norm = util::normalize_zip_path(raw_name);
    let norm = norm.trim_end_matches('/');
    if norm.is_empty() {
        return None;
    }
    let rel = if keep_paths {
        let mut buf = PathBuf::new();
        for part in norm.split('/') {
            match part {
                "" | "." => continue,
                ".." => return None, // 拒绝路径穿越
                p => {
                    if p.contains(':') {
                        return None;
                    }
                    buf.push(p);
                }
            }
        }
        buf
    } else {
        PathBuf::from(util::base_name(norm))
    };
    if rel.as_os_str().is_empty() {
        return None;
    }
    Some(dest.join(rel))
}

fn map_zip_err(e: ZipError, name: &str) -> anyhow::Error {
    match e {
        ZipError::InvalidPassword => anyhow::anyhow!("密码错误，无法解压「{name}」"),
        ZipError::UnsupportedArchive(msg) if msg == ZipError::PASSWORD_REQUIRED => {
            anyhow::anyhow!("「{name}」已加密，需要密码")
        }
        other => anyhow::anyhow!("解压「{name}」失败：{other}"),
    }
}

/// 解压压缩包（可指定子集与密码）。
pub fn extract(archive: &Path, opt: &ExtractOptions, rep: &Reporter) -> Result<String> {
    let metas = read_entries(archive)?;

    let wanted: Vec<&ArchiveEntry> = match &opt.selection {
        None => metas.iter().collect(),
        Some(sel) => metas
            .iter()
            .filter(|e| {
                sel.iter()
                    .any(|s| e.path == *s || e.path.starts_with(&format!("{s}/")))
            })
            .collect(),
    };

    if wanted.is_empty() {
        bail!("没有选中任何可解压的内容");
    }

    let total: u64 = wanted.iter().filter(|e| !e.is_dir).map(|e| e.size).sum();
    rep.total(total);
    fs::create_dir_all(&opt.dest)
        .with_context(|| format!("无法创建目标目录 {}", opt.dest.display()))?;

    let file = File::open(archive)?;
    let mut za = ZipArchive::new(BufReader::with_capacity(BUF, file))?;

    let mut buf = vec![0u8; BUF];
    let mut done = 0u64;
    let mut written = 0usize;
    let mut skipped = 0usize;

    for meta in wanted {
        rep.check_cancel()?;
        let Some(out_path) = safe_out_path(&opt.dest, &meta.path, opt.keep_paths) else {
            skipped += 1;
            rep.log(format!("跳过不安全的路径：{}", meta.path));
            continue;
        };

        if meta.is_dir {
            if opt.keep_paths {
                fs::create_dir_all(&out_path)?;
            }
            continue;
        }

        rep.progress(done, &meta.path);

        let final_path = if out_path.exists() {
            match opt.overwrite {
                Overwrite::Skip => {
                    skipped += 1;
                    done += meta.size;
                    continue;
                }
                Overwrite::AutoRename => util::unique_path(&out_path),
                Overwrite::Always => out_path.clone(),
            }
        } else {
            out_path.clone()
        };

        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut reader = if meta.encrypted {
            let pw = opt
                .password
                .as_deref()
                .filter(|p| !p.is_empty())
                .ok_or_else(|| anyhow::anyhow!("「{}」已加密，需要密码", meta.path))?;
            za.by_index_decrypt(meta.index, pw.as_bytes())
                .map_err(|e| map_zip_err(e, &meta.path))?
        } else {
            za.by_index(meta.index)
                .map_err(|e| map_zip_err(e, &meta.path))?
        };

        let mut out = BufWriter::with_capacity(
            BUF,
            File::create(&final_path)
                .with_context(|| format!("无法写入 {}", final_path.display()))?,
        );

        loop {
            if rep.cancelled() {
                drop(out);
                let _ = fs::remove_file(&final_path);
                bail!("操作已被用户取消");
            }
            let n = match reader.read(&mut buf) {
                Ok(n) => n,
                Err(e) => {
                    drop(out);
                    let _ = fs::remove_file(&final_path);
                    return Err(anyhow::anyhow!(
                        "解压「{}」失败：{}（可能是密码错误或文件损坏）",
                        meta.path,
                        e
                    ));
                }
            };
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])?;
            done += n as u64;
            rep.progress(done, &meta.path);
        }
        out.flush()?;
        written += 1;
    }

    let mut msg = format!(
        "已解压 {} 个文件（{}）到 {}",
        written,
        util::format_size(total),
        opt.dest.display()
    );
    if skipped > 0 {
        msg.push_str(&format!("，跳过 {skipped} 个"));
    }
    Ok(msg)
}

/// 解压单个条目到临时目录，用于“查看”功能。
pub fn extract_one_to_temp(
    archive: &Path,
    entry_path: &str,
    password: Option<&str>,
) -> Result<PathBuf> {
    let metas = read_entries(archive)?;
    let meta = metas
        .iter()
        .find(|e| e.path == entry_path)
        .context("压缩包中找不到该条目")?;

    let tmp_dir = std::env::temp_dir()
        .join("rustrar")
        .join(format!("view_{}", std::process::id()));
    fs::create_dir_all(&tmp_dir)?;
    let out_path = tmp_dir.join(util::base_name(&meta.path));

    let file = File::open(archive)?;
    let mut za = ZipArchive::new(BufReader::with_capacity(BUF, file))?;
    let mut reader = if meta.encrypted {
        let pw = password
            .filter(|p| !p.is_empty())
            .ok_or_else(|| anyhow::anyhow!("该文件已加密，需要密码"))?;
        za.by_index_decrypt(meta.index, pw.as_bytes())
            .map_err(|e| map_zip_err(e, &meta.path))?
    } else {
        za.by_index(meta.index)
            .map_err(|e| map_zip_err(e, &meta.path))?
    };

    let mut out = BufWriter::with_capacity(BUF, File::create(&out_path)?);
    std::io::copy(&mut reader, &mut out)?;
    out.flush()?;
    Ok(out_path)
}

/// 测试压缩包完整性（逐条校验 CRC）。
pub fn test(archive: &Path, password: Option<&str>, rep: &Reporter) -> Result<String> {
    let metas = read_entries(archive)?;
    let files: Vec<&ArchiveEntry> = metas.iter().filter(|e| !e.is_dir).collect();
    let total: u64 = files.iter().map(|e| e.size).sum();
    rep.total(total);

    let file = File::open(archive)?;
    let mut za = ZipArchive::new(BufReader::with_capacity(BUF, file))?;

    let mut buf = vec![0u8; BUF];
    let mut done = 0u64;
    let mut ok = 0usize;
    let mut bad = 0usize;

    for meta in &files {
        rep.check_cancel()?;
        rep.progress(done, &meta.path);

        let reader = if meta.encrypted {
            let Some(pw) = password.filter(|p| !p.is_empty()) else {
                bad += 1;
                rep.log(format!("✗ {}：需要密码", meta.path));
                done += meta.size;
                continue;
            };
            za.by_index_decrypt(meta.index, pw.as_bytes())
        } else {
            za.by_index(meta.index)
        };

        let mut reader = match reader {
            Ok(r) => r,
            Err(e) => {
                bad += 1;
                rep.log(format!("✗ {}：{}", meta.path, e));
                done += meta.size;
                continue;
            }
        };

        let mut failed = false;
        loop {
            rep.check_cancel()?;
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    done += n as u64;
                    rep.progress(done, &meta.path);
                }
                Err(e) => {
                    failed = true;
                    rep.log(format!("✗ {}：{}", meta.path, e));
                    break;
                }
            }
        }
        if failed {
            bad += 1;
        } else {
            ok += 1;
        }
    }

    if bad == 0 {
        Ok(format!("测试完成：{ok} 个文件全部通过 CRC 校验"))
    } else {
        Ok(format!("测试完成：{ok} 个正常，{bad} 个有问题"))
    }
}
