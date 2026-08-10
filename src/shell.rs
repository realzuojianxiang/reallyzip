//! Windows 资源管理器右键菜单集成（写入 HKCU，无需管理员权限）。

#[cfg(windows)]
mod imp {
    use anyhow::{Context, Result};
    use winreg::RegKey;
    use winreg::enums::*;

    const ADD_FILE: &str = r"Software\Classes\*\shell\RustRAR.Add";
    const ADD_DIR: &str = r"Software\Classes\Directory\shell\RustRAR.Add";
    const ZIP_ROOT: &str = r"Software\Classes\SystemFileAssociations\.zip\shell";
    const BG_OPEN: &str = r"Software\Classes\Directory\Background\shell\RustRAR.Open";

    fn exe() -> Result<String> {
        Ok(std::env::current_exe()?.to_string_lossy().to_string())
    }

    fn make_verb(path: &str, title: &str, args: &str, exe: &str, position: Option<&str>) -> Result<()> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu
            .create_subkey(path)
            .with_context(|| format!("无法创建注册表项 {path}"))?;
        key.set_value("", &title)?;
        key.set_value("Icon", &format!("\"{exe}\",0"))?;
        if let Some(pos) = position {
            key.set_value("Position", &pos)?;
        }
        let (cmd, _) = key.create_subkey("command")?;
        cmd.set_value("", &format!("\"{exe}\" {args}"))?;
        Ok(())
    }

    pub fn register() -> Result<()> {
        let exe = exe()?;

        // 任意文件 / 文件夹：添加到压缩文件
        make_verb(ADD_FILE, "添加到压缩文件…(RustRAR)", "--compress \"%1\"", &exe, Some("Top"))?;
        make_verb(ADD_DIR, "添加到压缩文件…(RustRAR)", "--compress \"%1\"", &exe, Some("Top"))?;

        // .zip 文件专属动作
        make_verb(
            &format!(r"{ZIP_ROOT}\RustRAR.Open"),
            "用 RustRAR 打开",
            "\"%1\"",
            &exe,
            Some("Top"),
        )?;
        make_verb(
            &format!(r"{ZIP_ROOT}\RustRAR.ExtractTo"),
            "解压到…(RustRAR)",
            "--extract-to \"%1\"",
            &exe,
            None,
        )?;
        make_verb(
            &format!(r"{ZIP_ROOT}\RustRAR.ExtractHere"),
            "解压到当前文件夹(RustRAR)",
            "--extract-here \"%1\"",
            &exe,
            None,
        )?;

        // 文件夹空白处：打开 RustRAR
        make_verb(BG_OPEN, "在此处打开 RustRAR", "\"%V\"", &exe, None)?;

        notify_shell();
        Ok(())
    }

    pub fn unregister() -> Result<()> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        for path in [
            ADD_FILE,
            ADD_DIR,
            BG_OPEN,
            &format!(r"{ZIP_ROOT}\RustRAR.Open"),
            &format!(r"{ZIP_ROOT}\RustRAR.ExtractTo"),
            &format!(r"{ZIP_ROOT}\RustRAR.ExtractHere"),
        ] {
            let _ = hkcu.delete_subkey_all(path);
        }
        notify_shell();
        Ok(())
    }

    pub fn is_registered() -> bool {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        hkcu.open_subkey(ADD_FILE).is_ok()
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
}

pub use imp::{is_registered, register, unregister};
