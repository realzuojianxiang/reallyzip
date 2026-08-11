//! 诊断日志：右键菜单调用走的是 `windows_subsystem="windows"` 无窗口 GUI 子系统，
//! 所有 `println!` / `eprintln!` 都被静默丢弃，因此「右键压缩没反应、也没报错」时
//! 无法定位原因。本模块把关键步骤写入 `%TEMP%\reallyzip_debug.log`，便于精确定位。

use std::io::Write;

/// 可读时间戳（本地时间，精确到毫秒）。
fn stamp() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string()
}

/// 进程启动时调用一次：清空旧日志并记下进程/位置信息，方便只看最新一次右键操作。
pub fn reset() {
    let path = std::env::temp_dir().join("reallyzip_debug.log");
    let _ = std::fs::write(&path, "");
    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    log(&format!(
        "==== 进程启动 pid={} exe={} cwd={} ====",
        std::process::id(),
        exe,
        cwd
    ));
}

/// 追加一行日志。
pub fn log(line: &str) {
    let path = std::env::temp_dir().join("reallyzip_debug.log");
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut f| writeln!(f, "[{}] {}", stamp(), line));
}
