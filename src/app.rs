//! 主窗口：WinRAR 风格的文件管理 + 压缩包浏览。

use chrono::NaiveDateTime;
use egui::{Align, Layout, RichText};
use egui_extras::{Column, TableBuilder};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::archive::{self, CreateOptions, ExtractOptions, Level, OpenArchive, Overwrite};
use crate::cli::Startup;
use crate::shell;
use crate::task::{self, JobOutcome, RunningJob};
use crate::ui::icons::{self, IconKind, ToolIcon};
use crate::ui::theme;
use crate::util;
use crate::volume;

mod dialogs;

// ------------------------------------------------------------------ 数据

#[derive(Clone)]
struct Row {
    name: String,
    is_parent: bool,
    is_dir: bool,
    icon: IconKind,
    size: u64,
    packed: Option<u64>,
    modified: Option<NaiveDateTime>,
    crc: Option<u32>,
    encrypted: bool,
    kind: String,
    local_path: Option<PathBuf>,
    inner_path: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Col {
    Name,
    Size,
    Packed,
    Ratio,
    Kind,
    Modified,
    Crc,
}

impl Col {
    fn title(self) -> &'static str {
        match self {
            Col::Name => "名称",
            Col::Size => "大小",
            Col::Packed => "压缩后",
            Col::Ratio => "压缩率",
            Col::Kind => "类型",
            Col::Modified => "修改时间",
            Col::Crc => "CRC32",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SortKey {
    Name,
    Size,
    Kind,
    Modified,
}

#[derive(Clone, PartialEq)]
enum Dialog {
    None,
    Create,
    Extract,
    Password,
    Settings,
    About,
}

#[derive(Clone)]
enum Action {
    Up,
    Refresh,
    EnterLocal(PathBuf),
    OpenArchiveFile(PathBuf),
    EnterInner(String),
    OpenInnerFile(String),
    OpenLocalFile(PathBuf),
    CloseArchive,
    ShowCreate,
    ShowExtract { selected_only: bool },
    ExtractHere,
    Test,
    View,
    Delete,
    GoHome,
    SelectAll,
    InvertSelection,
    Reveal,
    PickOpen,
    PickFolder,
}

struct CreateDlg {
    sources: Vec<PathBuf>,
    dest: String,
    level: Level,
    use_password: bool,
    password: String,
    password2: String,
    show_pw: bool,
    split_idx: usize,
    split_custom_mb: String,
    append: bool,
}

impl Default for CreateDlg {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            dest: String::new(),
            level: Level::Normal,
            use_password: false,
            password: String::new(),
            password2: String::new(),
            show_pw: false,
            split_idx: 0,
            split_custom_mb: String::new(),
            append: false,
        }
    }
}

struct ExtractDlg {
    archive: PathBuf,
    dest: String,
    selection: Option<HashSet<String>>,
    keep_paths: bool,
    overwrite: Overwrite,
    password: String,
    show_pw: bool,
    needs_pw: bool,
    count_label: String,
}

impl Default for ExtractDlg {
    fn default() -> Self {
        Self {
            archive: PathBuf::new(),
            dest: String::new(),
            selection: None,
            keep_paths: true,
            overwrite: Overwrite::AutoRename,
            password: String::new(),
            show_pw: false,
            needs_pw: false,
            count_label: String::new(),
        }
    }
}

struct Preview {
    title: String,
    text: Option<String>,
    path: PathBuf,
    size: u64,
}

// ------------------------------------------------------------------ App

pub struct RustRarApp {
    cwd: PathBuf, // 空路径表示“此电脑”
    archive: Option<OpenArchive>,
    inner: String,
    rows: Vec<Row>,
    selected: Vec<bool>,
    anchor: Option<usize>,
    sort: SortKey,
    sort_asc: bool,
    filter: String,

    status: String,
    job: Option<RunningJob>,
    error: Option<String>,
    info: Option<String>,

    dlg: Dialog,
    create: CreateDlg,
    extract: ExtractDlg,
    pw_input: String,
    pw_show: bool,
    pw_hint: String,
    pending_after_pw: Option<Action>,
    password: Option<String>,
    preview: Option<Preview>,
    shell_registered: bool,

    pending: Vec<Action>,
    startup: Option<Startup>,
}

impl RustRarApp {
    pub fn new(startup: Startup) -> Self {
        let home = home_dir();
        Self {
            cwd: home,
            archive: None,
            inner: String::new(),
            rows: Vec::new(),
            selected: Vec::new(),
            anchor: None,
            sort: SortKey::Name,
            sort_asc: true,
            filter: String::new(),
            status: "就绪".to_string(),
            job: None,
            error: None,
            info: None,
            dlg: Dialog::None,
            create: CreateDlg::default(),
            extract: ExtractDlg::default(),
            pw_input: String::new(),
            pw_show: false,
            pw_hint: String::new(),
            pending_after_pw: None,
            password: None,
            preview: None,
            shell_registered: shell::is_registered(),
            pending: Vec::new(),
            startup: Some(startup),
        }
    }

    // ---------------------------------------------------------- 列表刷新

    fn refresh(&mut self) {
        let mut rows: Vec<Row> = Vec::new();

        match &self.archive {
            Some(ar) => {
                rows.push(Row {
                    name: "..".to_string(),
                    is_parent: true,
                    is_dir: true,
                    icon: IconKind::FolderUp,
                    size: 0,
                    packed: None,
                    modified: None,
                    crc: None,
                    encrypted: false,
                    kind: "上级目录".to_string(),
                    local_path: None,
                    inner_path: None,
                });
                for node in archive::children_of(&ar.entries, &self.inner) {
                    let icon = if node.is_dir {
                        IconKind::Folder
                    } else if node.encrypted {
                        IconKind::Locked
                    } else if util::is_archive_path(Path::new(&node.name)) {
                        IconKind::Archive
                    } else {
                        IconKind::File
                    };
                    rows.push(Row {
                        kind: util::file_kind(&node.name, node.is_dir),
                        name: node.name.clone(),
                        is_parent: false,
                        is_dir: node.is_dir,
                        icon,
                        size: node.size,
                        packed: Some(node.compressed),
                        modified: node.modified,
                        crc: node.crc32,
                        encrypted: node.encrypted,
                        local_path: None,
                        inner_path: Some(node.full),
                    });
                }
            }
            None => {
                if self.cwd.as_os_str().is_empty() {
                    for letter in b'A'..=b'Z' {
                        let p = PathBuf::from(format!("{}:\\", letter as char));
                        if p.exists() {
                            rows.push(Row {
                                name: format!("{}:", letter as char),
                                is_parent: false,
                                is_dir: true,
                                icon: IconKind::Folder,
                                size: 0,
                                packed: None,
                                modified: None,
                                crc: None,
                                encrypted: false,
                                kind: "本地磁盘".to_string(),
                                local_path: Some(p),
                                inner_path: None,
                            });
                        }
                    }
                } else {
                    rows.push(Row {
                        name: "..".to_string(),
                        is_parent: true,
                        is_dir: true,
                        icon: IconKind::FolderUp,
                        size: 0,
                        packed: None,
                        modified: None,
                        crc: None,
                        encrypted: false,
                        kind: "上级目录".to_string(),
                        local_path: None,
                        inner_path: None,
                    });

                    match std::fs::read_dir(&self.cwd) {
                        Ok(rd) => {
                            let mut dirs = Vec::new();
                            let mut files = Vec::new();
                            for entry in rd.flatten() {
                                let path = entry.path();
                                let name = entry.file_name().to_string_lossy().to_string();
                                let md = match entry.metadata() {
                                    Ok(m) => m,
                                    Err(_) => continue,
                                };
                                let modified =
                                    md.modified().ok().and_then(util::system_time_to_naive);
                                if md.is_dir() {
                                    dirs.push(Row {
                                        kind: "文件夹".to_string(),
                                        name,
                                        is_parent: false,
                                        is_dir: true,
                                        icon: IconKind::Folder,
                                        size: 0,
                                        packed: None,
                                        modified,
                                        crc: None,
                                        encrypted: false,
                                        local_path: Some(path),
                                        inner_path: None,
                                    });
                                } else {
                                    let is_ar = util::is_archive_path(&path);
                                    files.push(Row {
                                        kind: util::file_kind(&name, false),
                                        name,
                                        is_parent: false,
                                        is_dir: false,
                                        icon: if is_ar {
                                            IconKind::Archive
                                        } else {
                                            IconKind::File
                                        },
                                        size: md.len(),
                                        packed: None,
                                        modified,
                                        crc: None,
                                        encrypted: false,
                                        local_path: Some(path),
                                        inner_path: None,
                                    });
                                }
                            }
                            rows.extend(dirs);
                            rows.extend(files);
                        }
                        Err(e) => {
                            self.error = Some(format!("无法读取目录：{e}"));
                        }
                    }
                }
            }
        }

        // 过滤
        if !self.filter.trim().is_empty() {
            let f = self.filter.to_lowercase();
            rows.retain(|r| r.is_parent || r.name.to_lowercase().contains(&f));
        }

        // 排序（父目录固定第一，目录优先）
        let asc = self.sort_asc;
        let key = self.sort;
        let parent: Vec<Row> = rows.iter().filter(|r| r.is_parent).cloned().collect();
        let mut rest: Vec<Row> = rows.into_iter().filter(|r| !r.is_parent).collect();
        rest.sort_by(|a, b| {
            let dir_cmp = b.is_dir.cmp(&a.is_dir);
            if dir_cmp != std::cmp::Ordering::Equal {
                return dir_cmp;
            }
            let ord = match key {
                SortKey::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortKey::Size => a.size.cmp(&b.size),
                SortKey::Kind => a.kind.cmp(&b.kind),
                SortKey::Modified => a.modified.cmp(&b.modified),
            };
            if asc { ord } else { ord.reverse() }
        });

        let mut out = parent;
        out.extend(rest);
        self.selected = vec![false; out.len()];
        self.rows = out;
        self.anchor = None;
    }

    fn columns(&self) -> Vec<Col> {
        if self.archive.is_some() {
            vec![
                Col::Name,
                Col::Size,
                Col::Packed,
                Col::Ratio,
                Col::Kind,
                Col::Modified,
                Col::Crc,
            ]
        } else {
            vec![Col::Name, Col::Size, Col::Kind, Col::Modified]
        }
    }

    fn location_label(&self) -> String {
        match &self.archive {
            Some(ar) => {
                if self.inner.is_empty() {
                    format!("{}\\", ar.display_path.display())
                } else {
                    format!("{}\\{}", ar.display_path.display(), self.inner.replace('/', "\\"))
                }
            }
            None => {
                if self.cwd.as_os_str().is_empty() {
                    "此电脑".to_string()
                } else {
                    self.cwd.display().to_string()
                }
            }
        }
    }

    fn selected_rows(&self) -> Vec<&Row> {
        self.rows
            .iter()
            .zip(self.selected.iter())
            .filter(|(r, s)| **s && !r.is_parent)
            .map(|(r, _)| r)
            .collect()
    }

    fn selected_local_paths(&self) -> Vec<PathBuf> {
        self.selected_rows()
            .iter()
            .filter_map(|r| r.local_path.clone())
            .collect()
    }

    fn selected_inner(&self) -> HashSet<String> {
        self.selected_rows()
            .iter()
            .filter_map(|r| r.inner_path.clone())
            .collect()
    }

    // ---------------------------------------------------------- 任务

    fn busy(&self) -> bool {
        self.job.is_some()
    }

    fn start_open(&mut self, ctx: &egui::Context, path: PathBuf) {
        if self.busy() {
            return;
        }
        let p = path.clone();
        self.job = Some(task::spawn(
            ctx,
            format!("正在打开 {}", file_name_of(&path)),
            move |rep| {
                let ar = archive::open(&p, rep)?;
                let msg = format!(
                    "已打开 {}：{} 个文件，{} → {}",
                    file_name_of(&p),
                    ar.file_count(),
                    util::format_size(ar.total_size),
                    util::format_size(ar.total_packed)
                );
                Ok((msg, JobOutcome::Opened(Box::new(ar))))
            },
        ));
    }

    fn start_extract(&mut self, ctx: &egui::Context, archive_path: PathBuf, opt: ExtractOptions) {
        if self.busy() {
            return;
        }
        self.job = Some(task::spawn(ctx, "正在解压", move |rep| {
            let msg = archive::extract(&archive_path, &opt, rep)?;
            Ok((msg, JobOutcome::None))
        }));
    }

    fn start_create(
        &mut self,
        ctx: &egui::Context,
        sources: Vec<PathBuf>,
        dest: PathBuf,
        opt: CreateOptions,
        append: bool,
    ) {
        if self.busy() {
            return;
        }
        self.job = Some(task::spawn(ctx, "正在压缩", move |rep| {
            let msg = if append {
                archive::append(&dest, &sources, &opt, rep)?
            } else {
                archive::create(&sources, &dest, &opt, rep)?
            };
            Ok((msg, JobOutcome::None))
        }));
    }

    fn start_test(&mut self, ctx: &egui::Context) {
        let Some(ar) = &self.archive else { return };
        if self.busy() {
            return;
        }
        let path = ar.real_path.clone();
        let pw = self.password.clone();
        self.job = Some(task::spawn(ctx, "正在测试压缩包", move |rep| {
            let msg = archive::test(&path, pw.as_deref(), rep)?;
            Ok((msg, JobOutcome::None))
        }));
    }

    fn start_delete(&mut self, ctx: &egui::Context, targets: HashSet<String>) {
        let Some(ar) = &self.archive else { return };
        if self.busy() {
            return;
        }
        if !ar.volumes.is_empty() {
            self.error = Some("分卷压缩包不支持直接删除条目，请先解压后重新打包。".into());
            return;
        }
        let path = ar.real_path.clone();
        let reopen = ar.display_path.clone();
        self.job = Some(task::spawn(ctx, "正在删除条目", move |rep| {
            let msg = archive::delete_entries(&path, &targets, rep)?;
            let ar = archive::open(&reopen, rep)?;
            Ok((msg, JobOutcome::Opened(Box::new(ar))))
        }));
    }

    fn start_preview(&mut self, ctx: &egui::Context, entry: String) {
        let Some(ar) = &self.archive else { return };
        if self.busy() {
            return;
        }
        let path = ar.real_path.clone();
        let pw = self.password.clone();
        self.job = Some(task::spawn(ctx, "正在提取预览文件", move |rep| {
            rep.total(1);
            rep.progress(0, &entry);
            let out = archive::extract_one_to_temp(&path, &entry, pw.as_deref())?;
            rep.progress(1, &entry);
            Ok((String::new(), JobOutcome::Previewed(out, entry)))
        }));
    }

    fn poll_job(&mut self) {
        let Some(job) = &mut self.job else { return };
        if !job.poll() {
            return;
        }
        let result = job.take_result();
        self.job = None;
        match result {
            Some(Ok((msg, outcome))) => {
                match outcome {
                    JobOutcome::Opened(ar) => {
                        self.archive = Some(*ar);
                        self.inner.clear();
                        self.password = None;
                    }
                    JobOutcome::Previewed(path, name) => {
                        self.open_preview(path, name);
                    }
                    JobOutcome::None => {}
                }
                if !msg.is_empty() {
                    self.status = msg.clone();
                    self.info = Some(msg);
                }
                self.refresh();
            }
            Some(Err(e)) => {
                self.status = "操作失败".to_string();
                self.error = Some(e);
            }
            None => {}
        }
    }

    fn open_preview(&mut self, path: PathBuf, name: String) {
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let text = if size <= 4 * 1024 * 1024 {
            std::fs::read(&path).ok().and_then(|bytes| {
                if bytes.iter().take(8000).any(|b| *b == 0) {
                    None
                } else {
                    String::from_utf8(bytes).ok()
                }
            })
        } else {
            None
        };
        self.preview = Some(Preview {
            title: name,
            text,
            path,
            size,
        });
    }

    // ---------------------------------------------------------- 动作分发

    fn need_password(&self) -> bool {
        self.archive
            .as_ref()
            .map(|a| a.has_encrypted)
            .unwrap_or(false)
            && self.password.is_none()
    }

    fn ask_password(&mut self, hint: &str, then: Action) {
        self.pw_input.clear();
        self.pw_hint = hint.to_string();
        self.pending_after_pw = Some(then);
        self.dlg = Dialog::Password;
    }

    fn handle(&mut self, action: Action, ctx: &egui::Context) {
        match action {
            Action::Refresh => self.refresh(),
            Action::Up => {
                if self.archive.is_some() {
                    if self.inner.is_empty() {
                        self.close_archive();
                    } else {
                        self.inner = util::parent_dir(&self.inner);
                        self.refresh();
                    }
                } else if !self.cwd.as_os_str().is_empty() {
                    match self.cwd.parent() {
                        Some(p) if !p.as_os_str().is_empty() => {
                            self.cwd = p.to_path_buf();
                        }
                        _ => self.cwd = PathBuf::new(),
                    }
                    self.refresh();
                }
            }
            Action::GoHome => {
                self.close_archive();
                self.cwd = home_dir();
                self.refresh();
            }
            Action::EnterLocal(p) => {
                self.cwd = p;
                self.filter.clear();
                self.refresh();
            }
            Action::OpenArchiveFile(p) => {
                self.filter.clear();
                self.start_open(ctx, p);
            }
            Action::EnterInner(p) => {
                self.inner = p;
                self.filter.clear();
                self.refresh();
            }
            Action::OpenInnerFile(p) => {
                if self.need_password() {
                    self.ask_password("该压缩包已加密，请输入密码", Action::OpenInnerFile(p));
                } else {
                    self.start_preview(ctx, p);
                }
            }
            Action::OpenLocalFile(p) => util::open_with_system(&p),
            Action::CloseArchive => self.close_archive(),
            Action::ShowCreate => self.show_create_dialog(),
            Action::ShowExtract { selected_only } => self.show_extract_dialog(selected_only),
            Action::ExtractHere => self.quick_extract_here(ctx),
            Action::Test => {
                if self.need_password() {
                    self.ask_password("该压缩包已加密，请输入密码后测试", Action::Test);
                } else {
                    self.start_test(ctx);
                }
            }
            Action::View => {
                let target = self
                    .selected_rows()
                    .iter()
                    .find(|r| !r.is_dir)
                    .and_then(|r| r.inner_path.clone().or_else(|| r.local_path.as_ref().map(|p| p.display().to_string())));
                match (self.archive.is_some(), target) {
                    (true, Some(inner)) => self.handle(Action::OpenInnerFile(inner), ctx),
                    (false, Some(p)) => util::open_with_system(Path::new(&p)),
                    _ => self.error = Some("请先选择一个文件".into()),
                }
            }
            Action::Delete => self.do_delete(ctx),
            Action::SelectAll => {
                for (i, r) in self.rows.iter().enumerate() {
                    self.selected[i] = !r.is_parent;
                }
            }
            Action::InvertSelection => {
                for (i, r) in self.rows.iter().enumerate() {
                    if !r.is_parent {
                        self.selected[i] = !self.selected[i];
                    }
                }
            }
            Action::Reveal => {
                if let Some(p) = self.selected_local_paths().first() {
                    util::reveal_in_explorer(p);
                } else if let Some(ar) = &self.archive {
                    util::reveal_in_explorer(&ar.display_path);
                } else if !self.cwd.as_os_str().is_empty() {
                    util::open_with_system(&self.cwd.clone());
                }
            }
            Action::PickOpen => {
                if let Some(p) = rfd::FileDialog::new()
                    .add_filter("ZIP 压缩包", &["zip"])
                    .add_filter("分卷", &["001"])
                    .add_filter("所有文件", &["*"])
                    .pick_file()
                {
                    self.start_open(ctx, p);
                }
            }
            Action::PickFolder => {
                if let Some(p) = rfd::FileDialog::new().pick_folder() {
                    self.close_archive();
                    self.cwd = p;
                    self.refresh();
                }
            }
        }
    }

    fn close_archive(&mut self) {
        if let Some(ar) = self.archive.take() {
            if let Some(parent) = ar.display_path.parent() {
                self.cwd = parent.to_path_buf();
            }
        }
        self.inner.clear();
        self.password = None;
        self.refresh();
    }

    fn show_create_dialog(&mut self) {
        let sources = if self.archive.is_some() {
            // 压缩包内：向当前压缩包添加本地文件
            match rfd::FileDialog::new().pick_files() {
                Some(v) => v,
                None => return,
            }
        } else {
            let sel = self.selected_local_paths();
            if sel.is_empty() {
                match rfd::FileDialog::new().pick_files() {
                    Some(v) => v,
                    None => return,
                }
            } else {
                sel
            }
        };

        if sources.is_empty() {
            self.error = Some("请先选择要压缩的文件或文件夹".into());
            return;
        }

        let mut dlg = CreateDlg::default();
        if let Some(ar) = &self.archive {
            dlg.dest = ar.display_path.display().to_string();
            dlg.append = true;
        } else {
            let base = sources[0].clone();
            let parent = base.parent().map(|p| p.to_path_buf()).unwrap_or_default();
            let stem = if sources.len() == 1 {
                base.file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "archive".into())
            } else {
                parent
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "archive".into())
            };
            dlg.dest = parent.join(format!("{stem}.zip")).display().to_string();
        }
        dlg.sources = sources;
        self.create = dlg;
        self.dlg = Dialog::Create;
    }

    fn show_extract_dialog(&mut self, selected_only: bool) {
        // 本地视图下选中了压缩包 -> 解压它
        if self.archive.is_none() {
            let sel = self.selected_local_paths();
            let target = sel.iter().find(|p| util::is_archive_path(p)).cloned();
            let Some(target) = target else {
                self.error = Some("请选择一个 ZIP 压缩包，或先双击打开它".into());
                return;
            };
            let dest = target
                .parent()
                .map(|p| p.join(stem_without_zip(&target)))
                .unwrap_or_default();
            self.extract = ExtractDlg {
                archive: target,
                dest: dest.display().to_string(),
                selection: None,
                needs_pw: false,
                count_label: "全部内容".to_string(),
                ..Default::default()
            };
            self.dlg = Dialog::Extract;
            return;
        }

        let ar = self.archive.as_ref().unwrap();
        let selection = if selected_only {
            let s = self.selected_inner();
            if s.is_empty() { None } else { Some(s) }
        } else {
            None
        };
        let count_label = match &selection {
            Some(s) => format!("已选中 {} 项", s.len()),
            None => "全部内容".to_string(),
        };
        let dest = ar
            .display_path
            .parent()
            .map(|p| p.join(stem_without_zip(&ar.display_path)))
            .unwrap_or_default();

        self.extract = ExtractDlg {
            archive: ar.real_path.clone(),
            dest: dest.display().to_string(),
            selection,
            needs_pw: ar.has_encrypted,
            password: self.password.clone().unwrap_or_default(),
            count_label,
            ..Default::default()
        };
        self.dlg = Dialog::Extract;
    }

    fn quick_extract_here(&mut self, ctx: &egui::Context) {
        let (archive_path, dest, needs_pw) = match &self.archive {
            Some(ar) => (
                ar.real_path.clone(),
                ar.display_path
                    .parent()
                    .map(|p| p.join(stem_without_zip(&ar.display_path)))
                    .unwrap_or_default(),
                ar.has_encrypted,
            ),
            None => {
                let sel = self.selected_local_paths();
                let Some(target) = sel.iter().find(|p| util::is_archive_path(p)).cloned() else {
                    self.error = Some("请选择一个 ZIP 压缩包".into());
                    return;
                };
                let dest = target
                    .parent()
                    .map(|p| p.join(stem_without_zip(&target)))
                    .unwrap_or_default();
                (target, dest, false)
            }
        };

        if needs_pw && self.password.is_none() {
            self.ask_password("该压缩包已加密，请输入密码", Action::ExtractHere);
            return;
        }

        let opt = ExtractOptions {
            dest,
            selection: None,
            password: self.password.clone(),
            keep_paths: true,
            overwrite: Overwrite::AutoRename,
        };
        self.start_extract(ctx, archive_path, opt);
    }

    fn do_delete(&mut self, ctx: &egui::Context) {
        if self.archive.is_some() {
            let targets = self.selected_inner();
            if targets.is_empty() {
                self.error = Some("请先选择要从压缩包中删除的条目".into());
                return;
            }
            self.start_delete(ctx, targets);
        } else {
            let paths = self.selected_local_paths();
            if paths.is_empty() {
                self.error = Some("请先选择要删除的文件".into());
                return;
            }
            let names: Vec<String> = paths.iter().map(|p| file_name_of(p)).collect();
            let msg = format!(
                "确定要永久删除这 {} 项吗？\n\n{}\n\n此操作不可撤销。",
                paths.len(),
                names
                    .iter()
                    .take(12)
                    .map(|s| format!("· {s}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            let ok = rfd::MessageDialog::new()
                .set_title("删除确认")
                .set_description(msg)
                .set_level(rfd::MessageLevel::Warning)
                .set_buttons(rfd::MessageButtons::YesNo)
                .show();
            if ok == rfd::MessageDialogResult::Yes {
                let mut failed = 0;
                for p in &paths {
                    let r = if p.is_dir() {
                        std::fs::remove_dir_all(p)
                    } else {
                        std::fs::remove_file(p)
                    };
                    if r.is_err() {
                        failed += 1;
                    }
                }
                self.status = if failed == 0 {
                    format!("已删除 {} 项", paths.len())
                } else {
                    format!("删除完成，{failed} 项失败")
                };
                self.refresh();
            }
        }
    }

    fn apply_startup(&mut self, ctx: &egui::Context) {
        let Some(s) = self.startup.take() else { return };
        match s {
            Startup::Normal => {}
            Startup::Browse(p) => {
                self.cwd = p;
            }
            Startup::Open(p) => {
                if let Some(parent) = p.parent() {
                    self.cwd = parent.to_path_buf();
                }
                if util::is_archive_path(&p) {
                    self.start_open(ctx, p);
                } else {
                    util::open_with_system(&p);
                }
            }
            Startup::Compress(paths) => {
                if let Some(parent) = paths[0].parent() {
                    self.cwd = parent.to_path_buf();
                }
                let base = paths[0].clone();
                let parent = base.parent().map(|p| p.to_path_buf()).unwrap_or_default();
                let stem = base
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "archive".into());
                self.create = CreateDlg {
                    sources: paths,
                    dest: parent.join(format!("{stem}.zip")).display().to_string(),
                    ..Default::default()
                };
                self.dlg = Dialog::Create;
            }
            Startup::ExtractHere(p) => {
                if let Some(parent) = p.parent() {
                    self.cwd = parent.to_path_buf();
                }
                let dest = p
                    .parent()
                    .map(|d| d.join(stem_without_zip(&p)))
                    .unwrap_or_default();
                let opt = ExtractOptions {
                    dest,
                    selection: None,
                    password: None,
                    keep_paths: true,
                    overwrite: Overwrite::AutoRename,
                };
                self.start_extract(ctx, p, opt);
            }
            Startup::ExtractTo(p) => {
                if let Some(parent) = p.parent() {
                    self.cwd = parent.to_path_buf();
                }
                let dest = p
                    .parent()
                    .map(|d| d.join(stem_without_zip(&p)))
                    .unwrap_or_default();
                self.extract = ExtractDlg {
                    archive: p,
                    dest: dest.display().to_string(),
                    count_label: "全部内容".to_string(),
                    ..Default::default()
                };
                self.dlg = Dialog::Extract;
            }
            Startup::RegisterShell => {
                let _ = shell::register();
                self.shell_registered = shell::is_registered();
            }
            Startup::UnregisterShell => {
                let _ = shell::unregister();
                self.shell_registered = shell::is_registered();
            }
        }
        self.refresh();
    }
}

// ------------------------------------------------------------------ 绘制

impl eframe::App for RustRarApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.startup.is_some() {
            self.apply_startup(&ctx);
        }
        self.poll_job();
        self.handle_dropped(&ctx);
        self.handle_keys(&ctx);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::WINDOW_BG)
                    .inner_margin(egui::Margin::same(8)),
            )
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    egui::Frame::new()
                        .fill(theme::PANEL_BG)
                        .inner_margin(egui::Margin::symmetric(8, 4))
                        .show(ui, |ui| self.top_menu(ui));

                    ui.add_space(4.0);

                    egui::Frame::new()
                        .fill(theme::PANEL_BG)
                        .inner_margin(egui::Margin::symmetric(8, 4))
                        .show(ui, |ui| self.toolbar(ui));

                    ui.add_space(4.0);

                    egui::Frame::new()
                        .fill(theme::PANEL_BG)
                        .inner_margin(egui::Margin::symmetric(10, 4))
                        .show(ui, |ui| self.address_bar(ui));

                    ui.add_space(6.0);

                    let table_height = ui.available_height() - 30.0;
                    egui::Frame::new()
                        .fill(theme::PANEL_BG)
                        .inner_margin(egui::Margin::same(4))
                        .show(ui, |ui| {
                            ui.set_min_height(table_height.max(0.0));
                            egui::ScrollArea::vertical()
                                .auto_shrink([false; 2])
                                .show(ui, |ui| self.central(ui));
                        });

                    ui.add_space(6.0);

                    egui::Frame::new()
                        .fill(theme::PANEL_BG)
                        .inner_margin(egui::Margin::symmetric(12, 5))
                        .show(ui, |ui| self.status_bar(ui));
                });
            });

        self.dialogs(&ctx);
        self.message_windows(&ctx);

        let actions: Vec<Action> = self.pending.drain(..).collect();
        for a in actions {
            self.handle(a, &ctx);
        }
    }
}

impl RustRarApp {
    fn handle_dropped(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if dropped.is_empty() {
            return;
        }
        if dropped.len() == 1 && util::is_archive_path(&dropped[0]) {
            self.start_open(ctx, dropped[0].clone());
            return;
        }
        if dropped.len() == 1 && dropped[0].is_dir() {
            self.close_archive();
            self.cwd = dropped[0].clone();
            self.refresh();
            return;
        }
        let base = dropped[0].clone();
        let parent = base.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let stem = base
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "archive".into());
        self.create = CreateDlg {
            sources: dropped,
            dest: parent.join(format!("{stem}.zip")).display().to_string(),
            ..Default::default()
        };
        self.dlg = Dialog::Create;
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        if self.dlg != Dialog::None || self.preview.is_some() {
            return;
        }
        let (f5, back, ctrl_a, del, enter, esc) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::F5),
                i.key_pressed(egui::Key::Backspace),
                i.modifiers.ctrl && i.key_pressed(egui::Key::A),
                i.key_pressed(egui::Key::Delete),
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Escape),
            )
        });
        if f5 {
            self.pending.push(Action::Refresh);
        }
        if back {
            self.pending.push(Action::Up);
        }
        if ctrl_a {
            self.pending.push(Action::SelectAll);
        }
        if del {
            self.pending.push(Action::Delete);
        }
        if esc && self.archive.is_some() {
            self.pending.push(Action::CloseArchive);
        }
        if enter && let Some(row) = self.selected_rows().first() {
            self.pending.push(activate_action(row));
        }
    }

    fn top_menu(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_space(2.0);
            ui.menu_button("文件", |ui| {
                        if ui.button("打开压缩包…").clicked() {
                            self.pending.push(Action::PickOpen);
                            ui.close();
                        }
                        if ui.button("打开文件夹…").clicked() {
                            self.pending.push(Action::PickFolder);
                            ui.close();
                        }
                        ui.separator();
                        if ui
                            .add_enabled(self.archive.is_some(), egui::Button::new("关闭压缩包"))
                            .clicked()
                        {
                            self.pending.push(Action::CloseArchive);
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("退出").clicked() {
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.menu_button("命令", |ui| {
                        if ui.button("添加到压缩文件…").clicked() {
                            self.pending.push(Action::ShowCreate);
                            ui.close();
                        }
                        if ui.button("解压到…").clicked() {
                            self.pending.push(Action::ShowExtract {
                                selected_only: false,
                            });
                            ui.close();
                        }
                        if ui.button("解压选中项…").clicked() {
                            self.pending.push(Action::ShowExtract {
                                selected_only: true,
                            });
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("测试压缩包").clicked() {
                            self.pending.push(Action::Test);
                            ui.close();
                        }
                        if ui.button("查看").clicked() {
                            self.pending.push(Action::View);
                            ui.close();
                        }
                        if ui.button("删除").clicked() {
                            self.pending.push(Action::Delete);
                            ui.close();
                        }
                    });
                    ui.menu_button("工具", |ui| {
                        if ui.button("全选").clicked() {
                            self.pending.push(Action::SelectAll);
                            ui.close();
                        }
                        if ui.button("反选").clicked() {
                            self.pending.push(Action::InvertSelection);
                            ui.close();
                        }
                        ui.separator();
                        if ui.button("在资源管理器中显示").clicked() {
                            self.pending.push(Action::Reveal);
                            ui.close();
                        }
                        if ui.button("设置…").clicked() {
                            self.dlg = Dialog::Settings;
                            ui.close();
                        }
                    });
                    ui.menu_button("帮助", |ui| {
                        if ui.button("关于 RustRAR").clicked() {
                            self.dlg = Dialog::About;
                            ui.close();
                        }
                    });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if let Some(ar) = &self.archive {
                            let txt = if ar.volumes.is_empty() {
                                format!("压缩包：{}", ar.name())
                            } else {
                                format!("分卷压缩包：{}（{} 卷）", ar.name(), ar.volumes.len())
                            };
                            egui::Frame::new()
                                .fill(theme::ACCENT_SOFTER)
                                .corner_radius(egui::CornerRadius::same(4))
                                .inner_margin(egui::Margin::symmetric(8, 3))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(txt).color(theme::ACCENT).size(11.5));
                                });
                        }
                    });
                    ui.add_space(2.0);
                });
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let in_archive = self.archive.is_some();
        let busy = self.busy();
        ui.horizontal(|ui| {
            let add = icons::tool_button(ui, "添加", ToolIcon::Add, !busy);
                    if add.clicked() {
                        self.pending.push(Action::ShowCreate);
                    }
                    add.on_hover_text("把选中的文件压缩成 ZIP（Ctrl+N）");

                    let ex = icons::tool_button(ui, "解压到", ToolIcon::Extract, !busy);
                    if ex.clicked() {
                        self.pending.push(Action::ShowExtract {
                            selected_only: false,
                        });
                    }
                    ex.on_hover_text("解压压缩包到指定目录");

                    let test = icons::tool_button(ui, "测试", ToolIcon::Test, in_archive && !busy);
                    if test.clicked() {
                        self.pending.push(Action::Test);
                    }
                    test.on_hover_text("校验压缩包内所有文件的 CRC");

                    let view = icons::tool_button(ui, "查看", ToolIcon::View, !busy);
                    if view.clicked() {
                        self.pending.push(Action::View);
                    }
                    view.on_hover_text("预览选中的文件");

                    let del = icons::tool_button(ui, "删除", ToolIcon::Delete, !busy);
                    if del.clicked() {
                        self.pending.push(Action::Delete);
                    }
                    del.on_hover_text("删除选中项（Delete）");

                    ui.add_space(6.0);
                    vertical_separator(ui);
                    ui.add_space(6.0);

                    let up = icons::tool_button(ui, "上一级", ToolIcon::Up, !busy);
                    if up.clicked() {
                        self.pending.push(Action::Up);
                    }
                    let home = icons::tool_button(ui, "主目录", ToolIcon::Home, !busy);
                    if home.clicked() {
                        self.pending.push(Action::GoHome);
                    }
                    let refresh = icons::tool_button(ui, "刷新", ToolIcon::Refresh, !busy);
                    if refresh.clicked() {
                        self.pending.push(Action::Refresh);
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let info = icons::tool_button(ui, "信息", ToolIcon::Info, in_archive);
                        if info.clicked() {
                            if let Some(ar) = &self.archive {
                                self.info = Some(archive_info_text(ar));
                            }
                        }
                    });
                });
    }

    fn address_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("位置").color(theme::TEXT_DIM).size(12.0));
            let label = self.location_label();
            egui::Frame::new()
                        .fill(theme::PANEL_SOFT)
                        .stroke(egui::Stroke::new(1.0, theme::BORDER_STRONG))
                        .corner_radius(egui::CornerRadius::same(6))
                        .inner_margin(egui::Margin::symmetric(8, 4))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width() - 230.0);
                            let icon_rect = ui.allocate_space(egui::vec2(16.0, 16.0)).1;
                            icons::paint_in(
                                ui,
                                icon_rect,
                                if self.archive.is_some() {
                                    IconKind::Archive
                                } else {
                                    IconKind::Folder
                                },
                            );
                            ui.add(
                                egui::Label::new(RichText::new(label).size(12.5))
                                    .truncate(),
                            );
                        });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        egui::Frame::new()
                            .fill(theme::INPUT_BG)
                            .stroke(egui::Stroke::new(1.0, theme::BORDER))
                            .corner_radius(egui::CornerRadius::same(6))
                            .inner_margin(egui::Margin::symmetric(8, 3))
                            .show(ui, |ui| {
                                let resp = ui.add(
                                    egui::TextEdit::singleline(&mut self.filter)
                                        .hint_text("筛选名称…")
                                        .desired_width(170.0),
                                );
                                if resp.changed() {
                                    self.pending.push(Action::Refresh);
                                }
                            });
                    });
                });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        if let Some(job) = &self.job {
            let frac = job.fraction();
            ui.horizontal(|ui| {
                ui.label(RichText::new(&job.title).strong().size(12.5));
                ui.add(
                    egui::ProgressBar::new(frac)
                        .desired_width(260.0)
                        .show_percentage(),
                );
                let label = if job.label.len() > 60 {
                    format!("…{}", &job.label[job.label.len() - 58..])
                } else {
                    job.label.clone()
                };
                ui.label(RichText::new(label).color(theme::TEXT_DIM).size(12.0));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.small_button("取消").clicked() {
                        job.request_cancel();
                    }
                    if job.is_cancelling() {
                        ui.label(
                            RichText::new("正在取消…").color(theme::WARN).size(12.0),
                        );
                    }
                });
            });
            return;
        }

        ui.horizontal(|ui| {
            let sel: Vec<&Row> = self.selected_rows();
            let sel_size: u64 = sel.iter().map(|r| r.size).sum();
            let total_items = self.rows.iter().filter(|r| !r.is_parent).count();

            let left = if sel.is_empty() {
                format!("共 {total_items} 项")
            } else {
                format!(
                    "已选择 {} 项，共 {}（{} 字节）",
                    sel.len(),
                    util::format_size(sel_size),
                    util::format_thousands(sel_size)
                )
            };
            egui::Frame::new()
                .fill(theme::OK_GREEN_SOFT)
                .corner_radius(egui::CornerRadius::same(5))
                .inner_margin(egui::Margin::symmetric(8, 3))
                .show(ui, |ui| {
                    ui.label(RichText::new(left).color(theme::OK_GREEN).strong().size(12.0));
                });

            ui.add_space(6.0);
            ui.label(
                RichText::new(&self.status)
                    .color(theme::TEXT_DIM)
                    .size(12.0),
            );

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let Some(ar) = &self.archive {
                    let txt = format!(
                        "压缩包总计 {} → {}（{}）",
                        util::format_size(ar.total_size),
                        util::format_size(ar.total_packed),
                        util::ratio_str(ar.total_size, ar.total_packed)
                    );
                    ui.label(RichText::new(txt).color(theme::ACCENT).size(12.0));
                }
            });
        });
    }

    fn central(&mut self, ui: &mut egui::Ui) {
        let cols = self.columns();
        let mut click: Option<(usize, bool, bool)> = None; // idx, ctrl, shift
        let mut dbl: Option<usize> = None;
        let mut ctx_menu: Option<usize> = None;
        let mut sort_click: Option<SortKey> = None;

        let rows = &self.rows;

                let mut builder = TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .sense(egui::Sense::click())
                    .cell_layout(Layout::left_to_right(Align::Center))
                    .min_scrolled_height(0.0);

                for c in &cols {
                    builder = match c {
                        Col::Name => builder.column(Column::remainder().at_least(240.0).clip(true)),
                        Col::Size => builder.column(Column::initial(92.0).at_least(70.0)),
                        Col::Packed => builder.column(Column::initial(92.0).at_least(70.0)),
                        Col::Ratio => builder.column(Column::initial(64.0).at_least(50.0)),
                        Col::Kind => builder.column(Column::initial(120.0).at_least(70.0).clip(true)),
                        Col::Modified => builder.column(Column::initial(132.0).at_least(90.0)),
                        Col::Crc => builder.column(Column::initial(84.0).at_least(60.0)),
                    };
                }

                builder
                    .header(30.0, |mut header| {
                        for c in &cols {
                            header.col(|ui| {
                                // 表头单元格背景 + 底部清晰分割线
                                ui.painter().rect_filled(
                                    ui.max_rect(),
                                    egui::CornerRadius::same(6),
                                    theme::PANEL_SOFT,
                                );
                                let sep = egui::Rect::from_min_max(
                                    egui::pos2(ui.max_rect().left(), ui.max_rect().bottom() - 1.0),
                                    egui::pos2(ui.max_rect().right(), ui.max_rect().bottom()),
                                );
                                ui.painter().rect_filled(sep, 0.0, theme::BORDER);
                                let title = c.title();
                                let key = match c {
                                    Col::Name => Some(SortKey::Name),
                                    Col::Size => Some(SortKey::Size),
                                    Col::Kind => Some(SortKey::Kind),
                                    Col::Modified => Some(SortKey::Modified),
                                    _ => None,
                                };
                                let marker = match (key, self.sort, self.sort_asc) {
                                    (Some(k), s, true) if k == s => "  ▲",
                                    (Some(k), s, false) if k == s => "  ▼",
                                    _ => "",
                                };
                                ui.horizontal(|ui| {
                                    let resp = ui.add(
                                        egui::Label::new(
                                            RichText::new(format!("{title}"))
                                                .strong()
                                                .size(12.0)
                                                .color(theme::TEXT_DIM),
                                        )
                                        .sense(egui::Sense::click()),
                                    );
                                    if !marker.is_empty() {
                                        ui.label(
                                            RichText::new(marker)
                                                .size(10.0)
                                                .color(theme::ACCENT),
                                        );
                                    }
                                    if resp.clicked()
                                        && let Some(k) = key
                                    {
                                        sort_click = Some(k);
                                    }
                                });
                            });
                        }
                    })
                    .body(|body| {
                        body.rows(26.0, rows.len(), |mut tr| {
                            let idx = tr.index();
                            let row = &rows[idx];
                            tr.set_selected(self.selected[idx]);

                            for c in &cols {
                                tr.col(|ui| match c {
                                    Col::Name => {
                                        ui.add_space(3.0);
                                        let (_, r) = ui.allocate_space(egui::vec2(17.0, 17.0));
                                        icons::paint_in(ui, r, row.icon);
                                        ui.add_space(3.0);
                                        let mut txt = RichText::new(&row.name).size(13.0);
                                        if row.is_parent {
                                            txt = txt.color(theme::TEXT_DIM);
                                        }
                                        ui.add(egui::Label::new(txt).truncate());
                                        if row.encrypted {
                                            let (_, r) = ui.allocate_space(egui::vec2(14.0, 14.0));
                                            icons::paint_in(ui, r, icons::IconKind::Locked);
                                        }
                                    }
                                    Col::Size => {
                                        if !row.is_parent && !row.is_dir {
                                            right_label(ui, &util::format_size(row.size));
                                        } else if !row.is_parent && row.is_dir && row.size > 0 {
                                            right_label(ui, &util::format_size(row.size));
                                        }
                                    }
                                    Col::Packed => {
                                        if let Some(p) = row.packed
                                            && !row.is_parent
                                        {
                                            right_label(ui, &util::format_size(p));
                                        }
                                    }
                                    Col::Ratio => {
                                        if let Some(p) = row.packed
                                            && !row.is_parent
                                            && row.size > 0
                                        {
                                            right_label(ui, &util::ratio_str(row.size, p));
                                        }
                                    }
                                    Col::Kind => {
                                        if !row.is_parent {
                                            ui.add(
                                                egui::Label::new(
                                                    RichText::new(&row.kind)
                                                        .size(12.0)
                                                        .color(theme::TEXT_DIM),
                                                )
                                                .truncate(),
                                            );
                                        }
                                    }
                                    Col::Modified => {
                                        if !row.is_parent {
                                            ui.label(
                                                RichText::new(util::fmt_time(row.modified))
                                                    .size(12.0)
                                                    .color(theme::TEXT_DIM),
                                            );
                                        }
                                    }
                                    Col::Crc => {
                                        if let Some(c) = row.crc
                                            && !row.is_dir
                                        {
                                            ui.label(
                                                RichText::new(format!("{c:08X}"))
                                                    .size(11.5)
                                                    .family(egui::FontFamily::Monospace)
                                                    .color(theme::TEXT_DIM),
                                            );
                                        }
                                    }
                                });
                            }

                            let resp = tr.response();
                            if resp.clicked() {
                                let (ctrl, shift) =
                                    ui_input_mods(resp.ctx.input(|i| i.modifiers));
                                click = Some((idx, ctrl, shift));
                            }
                            if resp.double_clicked() {
                                dbl = Some(idx);
                            }
                            if resp.secondary_clicked() {
                                ctx_menu = Some(idx);
                                click = Some((idx, false, false));
                            }
                        });
                    });

        if let Some(k) = sort_click {
            if self.sort == k {
                self.sort_asc = !self.sort_asc;
            } else {
                self.sort = k;
                self.sort_asc = true;
            }
            self.refresh();
        }

        if let Some((idx, ctrl, shift)) = click {
            self.apply_click(idx, ctrl, shift);
        }
        if let Some(idx) = dbl
            && idx < self.rows.len()
        {
            let action = activate_action(&self.rows[idx]);
            self.pending.push(action);
        }
        if ctx_menu.is_some() {
            self.show_row_menu(ui.ctx());
        }
    }

    fn apply_click(&mut self, idx: usize, ctrl: bool, shift: bool) {
        if idx >= self.selected.len() {
            return;
        }
        if shift && let Some(a) = self.anchor {
            let (lo, hi) = if a <= idx { (a, idx) } else { (idx, a) };
            for s in self.selected.iter_mut() {
                *s = false;
            }
            for i in lo..=hi {
                if !self.rows[i].is_parent {
                    self.selected[i] = true;
                }
            }
            return;
        }
        if ctrl {
            self.selected[idx] = !self.selected[idx];
        } else {
            for s in self.selected.iter_mut() {
                *s = false;
            }
            self.selected[idx] = !self.rows[idx].is_parent;
        }
        self.anchor = Some(idx);
    }

    fn show_row_menu(&mut self, ctx: &egui::Context) {
        // 用一个跟随鼠标的小窗口模拟右键菜单
        let pos = ctx.input(|i| i.pointer.interact_pos()).unwrap_or_default();
        let in_archive = self.archive.is_some();
        egui::Area::new(egui::Id::new("row_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(180.0);
                    if in_archive {
                        if ui.button("解压选中项…").clicked() {
                            self.pending.push(Action::ShowExtract {
                                selected_only: true,
                            });
                        }
                        if ui.button("查看").clicked() {
                            self.pending.push(Action::View);
                        }
                        if ui.button("从压缩包中删除").clicked() {
                            self.pending.push(Action::Delete);
                        }
                    } else {
                        if ui.button("添加到压缩文件…").clicked() {
                            self.pending.push(Action::ShowCreate);
                        }
                        if ui.button("解压到…").clicked() {
                            self.pending.push(Action::ShowExtract {
                                selected_only: false,
                            });
                        }
                        if ui.button("解压到当前文件夹").clicked() {
                            self.pending.push(Action::ExtractHere);
                        }
                        ui.separator();
                        if ui.button("在资源管理器中显示").clicked() {
                            self.pending.push(Action::Reveal);
                        }
                        if ui.button("删除").clicked() {
                            self.pending.push(Action::Delete);
                        }
                    }
                });
            });
    }
}

fn right_label(ui: &mut egui::Ui, text: &str) {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.label(RichText::new(text).size(12.0));
    });
}

/// 工具栏中竖直方向的分隔线。
fn vertical_separator(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(1.0, 30.0), egui::Sense::hover());
    ui.painter().vline(rect.center().x, rect.y_range(), egui::Stroke::new(1.0, theme::BORDER));
}

fn ui_input_mods(m: egui::Modifiers) -> (bool, bool) {
    (m.ctrl || m.command, m.shift)
}

fn activate_action(row: &Row) -> Action {
    if row.is_parent {
        return Action::Up;
    }
    if let Some(inner) = &row.inner_path {
        if row.is_dir {
            Action::EnterInner(inner.clone())
        } else {
            Action::OpenInnerFile(inner.clone())
        }
    } else if let Some(p) = &row.local_path {
        if row.is_dir {
            Action::EnterLocal(p.clone())
        } else if util::is_archive_path(p) {
            Action::OpenArchiveFile(p.clone())
        } else {
            Action::OpenLocalFile(p.clone())
        }
    } else {
        Action::Refresh
    }
}

fn file_name_of(p: &Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| p.display().to_string())
}

/// `a.zip` -> `a`；`a.zip.001` -> `a`
fn stem_without_zip(p: &Path) -> String {
    let mut name = file_name_of(p);
    if volume::is_volume_part(Path::new(&name)) {
        name = name[..name.len().saturating_sub(4)].to_string();
    }
    if name.to_lowercase().ends_with(".zip") {
        name = name[..name.len() - 4].to_string();
    }
    if name.is_empty() {
        "extracted".to_string()
    } else {
        name
    }
}

fn home_dir() -> PathBuf {
    #[cfg(windows)]
    let key = "USERPROFILE";
    #[cfg(not(windows))]
    let key = "HOME";
    std::env::var(key)
        .map(PathBuf::from)
        .ok()
        .filter(|p| p.exists())
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn archive_info_text(ar: &OpenArchive) -> String {
    let mut s = String::new();
    s.push_str(&format!("压缩包：{}\n", ar.display_path.display()));
    s.push_str(&format!("条目总数：{}\n", ar.entries.len()));
    s.push_str(&format!("文件数：{}\n", ar.file_count()));
    s.push_str(&format!(
        "原始大小：{}（{} 字节）\n",
        util::format_size(ar.total_size),
        util::format_thousands(ar.total_size)
    ));
    s.push_str(&format!(
        "压缩后：{}（{} 字节）\n",
        util::format_size(ar.total_packed),
        util::format_thousands(ar.total_packed)
    ));
    s.push_str(&format!(
        "压缩率：{}\n",
        util::ratio_str(ar.total_size, ar.total_packed)
    ));
    s.push_str(&format!(
        "加密：{}\n",
        if ar.has_encrypted {
            "是（AES / ZipCrypto）"
        } else {
            "否"
        }
    ));
    if !ar.volumes.is_empty() {
        s.push_str(&format!("分卷数：{}\n", ar.volumes.len()));
    }
    s
}
