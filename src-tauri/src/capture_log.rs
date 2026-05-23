//! Python 取词诊断日志：优先写在 exe 同目录，便于在「安装文件夹」里直接找到。

use crate::app_data::app_data_dir;
use std::io::Write;
use std::path::PathBuf;

const LOG_NAME: &str = "python-capture.log";

/// 安装目录旁 logs（用户最容易找到）
pub fn install_log_file() -> PathBuf {
    install_log_dir().join(LOG_NAME)
}

fn strip_extended_prefix(path: &std::path::Path) -> PathBuf {
    let s = path.to_string_lossy();
    let stripped = s
        .strip_prefix(r"\\?\UNC\")
        .map(|rest| format!(r"\\{rest}"))
        .or_else(|| s.strip_prefix(r"\\?\").map(|rest| rest.to_string()))
        .unwrap_or_else(|| s.into_owned());
    PathBuf::from(stripped)
}

pub fn install_log_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| strip_extended_prefix(d).join("logs")))
        .unwrap_or_else(|| PathBuf::from("logs"))
}

/// 配置/历史同级的 AppData 目录（备用）
pub fn appdata_log_file() -> PathBuf {
    app_data_dir().join("logs").join(LOG_NAME)
}

pub fn all_log_files() -> Vec<PathBuf> {
    let mut v = vec![install_log_file()];
    let app = appdata_log_file();
    if app != v[0] {
        v.push(app);
    }
    v
}

pub fn paths_for_ui() -> Vec<String> {
    all_log_files()
        .iter()
        .map(|p| p.display().to_string())
        .collect()
}

pub fn log_file_hint_block() -> String {
    let lines: Vec<String> = paths_for_ui()
        .iter()
        .enumerate()
        .map(|(i, p)| {
            if i == 0 {
                format!("主日志（安装目录）: {p}")
            } else {
                format!("备用日志（AppData）: {p}")
            }
        })
        .collect();
    lines.join("\n")
}

pub fn append(block: &str) {
    for path in all_log_files() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let _ = writeln!(f, "{block}");
        }
    }
}

pub fn open_logs_folder() -> Result<String, String> {
    let dir = install_log_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建日志目录失败: {e}"))?;
    let dir_str = dir.display().to_string();
    open_in_explorer(&dir)?;
    Ok(dir_str)
}

#[cfg(windows)]
fn open_in_explorer(path: &std::path::Path) -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(path)
        .spawn()
        .map_err(|e| format!("无法打开资源管理器: {e}"))?;
    Ok(())
}

#[cfg(not(windows))]
fn open_in_explorer(path: &std::path::Path) -> Result<(), String> {
    std::process::Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map_err(|e| format!("无法打开文件夹: {e}"))?;
    Ok(())
}
