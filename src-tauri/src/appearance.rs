use crate::app_data::app_data_dir;
use crate::settings::TranslateSettings;
use std::fs;
use std::path::PathBuf;

const BG_DIR: &str = "backgrounds";
const BG_BASENAME: &str = "bubble-bg";

pub fn backgrounds_dir() -> PathBuf {
    app_data_dir().join(BG_DIR)
}

pub fn bubble_background_path(settings: &TranslateSettings) -> Option<PathBuf> {
    let name = settings.bubble_background.trim();
    if name.is_empty() {
        return None;
    }
    let name = name;
    let path = backgrounds_dir().join(name);
    if path.is_file() {
        Some(path)
    } else {
        None
    }
}

pub fn bubble_background_display_path(settings: &TranslateSettings) -> Option<String> {
    bubble_background_path(settings).map(|p| p.to_string_lossy().into_owned())
}

fn mime_for_path(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("bmp") => "image/bmp",
        _ => "image/png",
    }
}

/// 将本地图片读为 data URL，供 WebView 直接用作 CSS 背景（不依赖 asset 协议）
pub fn path_to_data_url(path: &std::path::Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }
    let bytes = fs::read(path).ok()?;
    if bytes.is_empty() {
        return None;
    }
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let mime = mime_for_path(path);
    Some(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}

pub fn bubble_background_data_url(settings: &TranslateSettings) -> Option<String> {
    bubble_background_path(settings).and_then(|p| path_to_data_url(&p))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PickBackgroundResult {
    pub filename: String,
    pub full_path: String,
}

pub fn pick_and_save_bubble_background() -> Result<PickBackgroundResult, String> {
    let picked = rfd::FileDialog::new()
        .add_filter("图片", &["png", "jpg", "jpeg", "webp", "gif", "bmp"])
        .pick_file()
        .ok_or_else(|| "未选择文件".to_string())?;

    let mut ext = picked
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_else(|| "png".to_string());
    const ALLOWED: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "bmp"];
    if !ALLOWED.contains(&ext.as_str()) {
        ext = "png".to_string();
    }

    let dir = backgrounds_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    for old in fs::read_dir(&dir).into_iter().flatten().flatten() {
        let name = old.file_name().to_string_lossy().into_owned();
        if name.starts_with(BG_BASENAME) {
            let _ = fs::remove_file(old.path());
        }
    }

    let filename = format!("{BG_BASENAME}.{ext}");
    let dest = dir.join(&filename);
    fs::copy(&picked, &dest).map_err(|e| format!("保存背景图失败: {e}"))?;
    Ok(PickBackgroundResult {
        full_path: dest.to_string_lossy().into_owned(),
        filename,
    })
}

pub fn clear_bubble_background_files() -> Result<(), String> {
    let dir = backgrounds_dir();
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(BG_BASENAME) {
            let _ = fs::remove_file(entry.path());
        }
    }
    Ok(())
}

pub fn validate_background_name(name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    if name.contains("..") || name.contains('/') || name.contains('\\') {
        return None;
    }
    let path = backgrounds_dir().join(name);
    if path.is_file() {
        Some(name.to_string())
    } else {
        None
    }
}
