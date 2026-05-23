//! 安装版与开发版各自使用独立数据目录，避免共用 %APPDATA% 下的旧缓存/历史。

use std::path::PathBuf;

/// 正式安装包使用的配置目录名（与开发时「果粒橙工具箱」分离）
pub const APP_DIR_NAME: &str = "GLC Quick Translate";

fn dir_name() -> &'static str {
    if cfg!(debug_assertions) {
        "果粒橙工具箱"
    } else {
        APP_DIR_NAME
    }
}

pub fn app_data_dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(dir_name())
}
