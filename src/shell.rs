//! Windows 资源管理器右键菜单集成（写入 HKCU，无需管理员权限）。
//!
//! 采用 Windows 7+ 的 `ExtendedSubCommandsKey` 级联菜单：右键只出现一个
//! 「ReallyZip」入口，展开后才是具体动作，不污染顶层菜单。
//!
//! 结构示意：
//! ```text
//! HKCU\Software\Classes\*\shell\ReallyZip          (顶级入口，无 command)
//!     MUIVerb                = ReallyZip
//!     ExtendedSubCommandsKey = ReallyZip.FileMenu  ← 相对 HKEY_CLASSES_ROOT
//!
//! HKCU\Software\Classes\ReallyZip.FileMenu\shell   (子菜单，按键名排序)
//!     01Add   → 添加到压缩文件…
//!     02AddTo → 压缩为 ZIP
//! ```

#[cfg(windows)]
mod imp {
    use anyhow::{Context, Result};
    use winreg::RegKey;
    use winreg::enums::*;

    /// 子菜单根键名（`ExtendedSubCommandsKey` 的值按 HKEY_CLASSES_ROOT 解析）。
    const MENU_FILE: &str = "ReallyZip.FileMenu";
    const MENU_ZIP: &str = "ReallyZip.ZipMenu";

    /// 四个顶级入口。
    const ENTRY_FILE: &str = r"Software\Classes\*\shell\ReallyZip";
    const ENTRY_DIR: &str = r"Software\Classes\Directory\shell\ReallyZip";
    const ENTRY_ZIP: &str = r"Software\Classes\SystemFileAssociations\.zip\shell\ReallyZip";
    const ENTRY_BG: &str = r"Software\Classes\Directory\Background\shell\ReallyZip.Open";

    /// 历史版本（RustRAR → RustZip → ReallyZip）留下的扁平菜单项，
    /// 每次注册/取消注册都清一遍，避免右键出现多套孤儿菜单。
    const LEGACY_KEYS: &[&str] = &[
        // 第一代：RustRAR
        r"Software\Classes\*\shell\RustRAR.Add",
        r"Software\Classes\Directory\shell\RustRAR.Add",
        r"Software\Classes\Directory\Background\shell\RustRAR.Open",
        r"Software\Classes\SystemFileAssociations\.zip\shell\RustRAR.Open",
        r"Software\Classes\SystemFileAssociations\.zip\shell\RustRAR.ExtractTo",
        r"Software\Classes\SystemFileAssociations\.zip\shell\RustRAR.ExtractHere",
        // 第二代：RustZip
        r"Software\Classes\*\shell\RustZip.Add",
        r"Software\Classes\Directory\shell\RustZip.Add",
        r"Software\Classes\Directory\Background\shell\RustZip.Open",
        r"Software\Classes\SystemFileAssociations\.zip\shell\RustZip.Open",
        r"Software\Classes\SystemFileAssociations\.zip\shell\RustZip.ExtractTo",
        r"Software\Classes\SystemFileAssociations\.zip\shell\RustZip.ExtractHere",
    ];

    fn menu_root(name: &str) -> String {
        format!(r"Software\Classes\{name}")
    }

    /// 删除历史版本遗留的右键菜单项（不存在则静默跳过）。
    fn purge_legacy() {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        for path in LEGACY_KEYS {
            let _ = hkcu.delete_subkey_all(path);
        }
    }

    /// 删除本版自己写过的键，保证重复注册时是全新结构而非增量叠加。
    fn purge_own() {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        for path in [
            ENTRY_FILE,
            ENTRY_DIR,
            ENTRY_ZIP,
            ENTRY_BG,
            &menu_root(MENU_FILE),
            &menu_root(MENU_ZIP),
        ] {
            let _ = hkcu.delete_subkey_all(path);
        }
    }

    /// 稳定安装目录：%LOCALAPPDATA%\ReallyZip
    fn install_root() -> std::path::PathBuf {
        let local = std::env::var("LOCALAPPDATA")
            .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().to_string());
        std::path::Path::new(&local).join("ReallyZip")
    }

    /// 返回用于注册的可执行文件路径。
    ///
    /// 若当前 exe 不在稳定安装目录，先把自身复制过去（保证注册表指向固定位置，
    /// 以后移动/重命名下载目录里的原 exe，右键菜单也不会失效），返回稳定副本路径；
    /// 若已在稳定目录，直接返回自身路径。复制失败时退回当前路径，至少能注册上。
    fn exe_for_register() -> String {
        let current = match std::env::current_exe() {
            Ok(p) => p,
            Err(_) => return String::new(),
        };
        let target = install_root().join("reallyzip.exe");
        if current.to_string_lossy().to_lowercase() == target.to_string_lossy().to_lowercase() {
            return current.to_string_lossy().to_string();
        }
        if std::fs::create_dir_all(install_root()).is_ok()
            && std::fs::copy(&current, &target).is_ok()
        {
            return target.to_string_lossy().to_string();
        }
        current.to_string_lossy().to_string()
    }

    /// 读取已注册的可执行文件路径（用于 UI 展示「注册位置」），未注册返回 None。
    pub fn registered_exe() -> Option<String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let path = format!(r"{MENU_FILE}\shell\02AddTo\command");
        let v: String = hkcu.open_subkey(&path).ok()?.get_value("").ok()?;
        // 命令形如 "C:\x\reallyzip.exe" --compress-here %*
        let start = v.find('"')?;
        let rest = &v[start + 1..];
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    }

    /// 写一个可执行动作（带 command 子键）。
    ///
    /// `multi` 为真时使用 Player 多选模型：一次选中多个文件只启动一个实例，
    /// 全部路径通过 `%*` 传入，用于「添加到压缩文件」这类需要合并处理的动作。
    fn write_verb(path: &str, title: &str, args: &str, exe: &str, multi: bool) -> Result<()> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu
            .create_subkey(path)
            .with_context(|| format!("无法创建注册表项 {path}"))?;
        key.set_value("MUIVerb", &title)?;
        key.set_value("Icon", &format!("\"{exe}\",0"))?;
        if multi {
            key.set_value("MultiSelectModel", &"Player")?;
        }
        let (cmd, _) = key.create_subkey("command")?;
        cmd.set_value("", &format!("\"{exe}\" {args}"))?;
        Ok(())
    }

    /// 写一个级联入口（没有 command 子键，指向子菜单根键）。
    fn write_cascade(path: &str, submenu: &str, exe: &str, applies_to: Option<&str>) -> Result<()> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu
            .create_subkey(path)
            .with_context(|| format!("无法创建注册表项 {path}"))?;
        key.set_value("MUIVerb", &"ReallyZip")?;
        key.set_value("Icon", &format!("\"{exe}\",0"))?;
        key.set_value("ExtendedSubCommandsKey", &submenu)?;
        key.set_value("Position", &"Top")?;
        if let Some(a) = applies_to {
            key.set_value("AppliesTo", &a)?;
        }
        Ok(())
    }

    pub fn register() -> Result<()> {
        let exe = exe_for_register();
        purge_legacy();
        purge_own();

        // ---- 子菜单 A：普通文件与文件夹 ----
        let a = menu_root(MENU_FILE);
        write_verb(
            &format!(r"{a}\shell\01Add"),
            "添加到压缩文件…",
            "--compress %*",
            &exe,
            true,
        )?;
        write_verb(
            &format!(r"{a}\shell\02AddTo"),
            "压缩为 ZIP",
            "--compress-here %*",
            &exe,
            true,
        )?;

        // ---- 子菜单 B：.zip 文件 ----
        let b = menu_root(MENU_ZIP);
        write_verb(
            &format!(r"{b}\shell\01Open"),
            "用 ReallyZip 打开",
            "\"%1\"",
            &exe,
            false,
        )?;
        write_verb(
            &format!(r"{b}\shell\02ExtractTo"),
            "解压到…",
            "--extract-to \"%1\"",
            &exe,
            false,
        )?;
        write_verb(
            &format!(r"{b}\shell\03ExtractHere"),
            "解压到当前文件夹",
            "--extract-here \"%1\"",
            &exe,
            false,
        )?;
        write_verb(
            &format!(r"{b}\shell\04Add"),
            "添加到压缩文件…",
            "--compress %*",
            &exe,
            true,
        )?;

        // ---- 顶级入口 ----
        // 通配入口排除 .zip，否则压缩包上会同时出现两个 ReallyZip 菜单
        write_cascade(
            ENTRY_FILE,
            MENU_FILE,
            &exe,
            Some("NOT System.FileName:\"*.zip\""),
        )?;
        write_cascade(ENTRY_DIR, MENU_FILE, &exe, None)?;
        write_cascade(ENTRY_ZIP, MENU_ZIP, &exe, None)?;
        // 文件夹空白处只有一个动作，扁平呈现即可
        write_verb(ENTRY_BG, "在此处打开 ReallyZip", "\"%V\"", &exe, false)?;

        notify_shell();
        Ok(())
    }

    pub fn unregister() -> Result<()> {
        purge_own();
        purge_legacy();
        // 清理稳定安装目录里的副本，避免留下孤立文件
        let _ = std::fs::remove_dir_all(install_root());
        notify_shell();
        Ok(())
    }

    pub fn is_registered() -> bool {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        hkcu.open_subkey(ENTRY_FILE).is_ok()
    }

    fn notify_shell() {
        // 让资源管理器尽快感知注册表变化
        unsafe extern "system" {
            fn SHChangeNotify(
                w_event_id: i32,
                u_flags: u32,
                dw_item1: *const core::ffi::c_void,
                dw_item2: *const core::ffi::c_void,
            );
        }
        const SHCNE_ASSOCCHANGED: i32 = 0x0800_0000;
        const SHCNF_IDLIST: u32 = 0x0000;
        unsafe {
            SHChangeNotify(
                SHCNE_ASSOCCHANGED,
                SHCNF_IDLIST,
                std::ptr::null(),
                std::ptr::null(),
            );
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use anyhow::{Result, bail};
    pub fn register() -> Result<()> {
        bail!("右键菜单集成目前仅支持 Windows")
    }
    pub fn unregister() -> Result<()> {
        bail!("右键菜单集成目前仅支持 Windows")
    }
    pub fn is_registered() -> bool {
        false
    }
    pub fn registered_exe() -> Option<String> {
        None
    }
}

pub use imp::{is_registered, registered_exe, register, unregister};
