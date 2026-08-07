//! 安装版与开发版各自使用独立数据目录，避免共用 %APPDATA% 下的旧缓存/历史。

use std::path::PathBuf;

pub const APP_DIR_NAME: &str = "GLC Quick Translate";

fn dir_name() -> &'static str {
    APP_DIR_NAME
}

/// 工程根目录（`quick-translate/src-tauri` 的上一级）
pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".."))
}

pub fn app_data_dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(dir_name())
}
