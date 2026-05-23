use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BubbleBgLayout {
    pub pos_x: u8,
    pub pos_y: u8,
    pub zoom: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct BubbleBgLayouts {
    pub loading: BubbleBgLayout,
    pub success: BubbleBgLayout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TranslateSettings {
    pub enabled: bool,
    /// 翻译并显示气泡
    pub hotkey: String,
    /// 翻译并用译文替换选中内容
    pub replace_hotkey: String,
    pub restore_clipboard: bool,
    pub copy_delay_ms: u64,
    pub target_lang: String,
    pub primary_provider: String,
    pub fallback_enabled: bool,
    pub timeout_sec: u64,
    pub cache_ttl_sec: u64,
    pub debug_scraper: bool,
    pub bubble_auto_close_sec: u64,
    pub use_python_capture: bool,
    /// 应用主题：dark | light
    #[serde(alias = "bubbleTheme")]
    pub app_theme: String,
    /// 翻译气泡卡片不透明度 50–100
    pub bubble_opacity: u8,
    /// 气泡背景图文件名（位于 AppData/backgrounds/）
    #[serde(default)]
    pub bubble_background: String,
    /// 各气泡形态的独立背景裁切
    #[serde(default)]
    pub bubble_bg_layouts: BubbleBgLayouts,
    /// 气泡主文字色 #RRGGBB，空=跟随主题
    #[serde(default)]
    pub bubble_text_color: String,
    #[serde(default)]
    pub bubble_muted_color: String,
    /// 历史记录保留条数
    #[serde(default = "default_history_max")]
    pub history_max_count: u32,
    /// 开机自动启动（Windows 注册表）
    #[serde(default)]
    pub launch_at_startup: bool,
}

fn default_history_max() -> u32 {
    200
}

impl Default for BubbleBgLayout {
    fn default() -> Self {
        Self {
            pos_x: 50,
            pos_y: 50,
            zoom: 100,
        }
    }
}

impl Default for TranslateSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            hotkey: "Ctrl+T".to_string(),
            replace_hotkey: "Ctrl+Shift+T".to_string(),
            restore_clipboard: true,
            copy_delay_ms: 200,
            target_lang: "auto".to_string(),
            primary_provider: "baidu".to_string(),
            fallback_enabled: true,
            timeout_sec: 20,
            cache_ttl_sec: 30,
            debug_scraper: false,
            bubble_auto_close_sec: 12,
            use_python_capture: true,
            app_theme: "light".to_string(),
            bubble_opacity: 92,
            bubble_background: String::new(),
            bubble_bg_layouts: BubbleBgLayouts::default(),
            bubble_text_color: String::new(),
            bubble_muted_color: String::new(),
            history_max_count: default_history_max(),
            launch_at_startup: false,
        }
    }
}

pub fn clamp_history_max(count: u32) -> u32 {
    count.clamp(20, 500)
}

pub fn normalize_app_theme(theme: &str) -> String {
    if theme.eq_ignore_ascii_case("light") {
        "light".to_string()
    } else {
        "dark".to_string()
    }
}

pub fn clamp_opacity(opacity: u8) -> u8 {
    opacity.clamp(50, 100)
}

pub fn clamp_bg_pos(pos: u8) -> u8 {
    pos.min(100)
}

pub fn clamp_bg_zoom(zoom: u8) -> u8 {
    zoom.clamp(50, 250)
}

pub fn normalize_bg_layout(mut l: BubbleBgLayout) -> BubbleBgLayout {
    l.pos_x = clamp_bg_pos(l.pos_x);
    l.pos_y = clamp_bg_pos(l.pos_y);
    l.zoom = clamp_bg_zoom(l.zoom);
    l
}

pub fn normalize_optional_hex_color(color: &str) -> String {
    let c = color.trim();
    if c.is_empty() {
        return String::new();
    }
    let hex = c.strip_prefix('#').unwrap_or(c);
    let ok = (hex.len() == 6 || hex.len() == 3) && hex.chars().all(|ch| ch.is_ascii_hexdigit());
    if ok {
        if c.starts_with('#') {
            c.to_string()
        } else {
            format!("#{c}")
        }
    } else {
        String::new()
    }
}

pub fn normalize_bg_layouts(mut layouts: BubbleBgLayouts) -> BubbleBgLayouts {
    layouts.loading = normalize_bg_layout(layouts.loading);
    layouts.success = normalize_bg_layout(layouts.success);
    layouts
}

/// 已移除的引擎回退为百度
pub fn normalize_primary_provider(id: &str) -> String {
    if id == "google" {
        "baidu".to_string()
    } else {
        id.to_string()
    }
}

pub fn normalize_settings(mut s: TranslateSettings) -> TranslateSettings {
    s.primary_provider = normalize_primary_provider(&s.primary_provider);
    s.app_theme = normalize_app_theme(&s.app_theme);
    s.bubble_opacity = clamp_opacity(s.bubble_opacity);
    s.bubble_background = crate::appearance::validate_background_name(&s.bubble_background)
        .unwrap_or_default();
    s.bubble_bg_layouts = normalize_bg_layouts(s.bubble_bg_layouts);
    s.bubble_text_color = normalize_optional_hex_color(&s.bubble_text_color);
    s.bubble_muted_color = normalize_optional_hex_color(&s.bubble_muted_color);
    s.history_max_count = clamp_history_max(s.history_max_count);
    s
}

pub fn config_path() -> PathBuf {
    crate::app_data::app_data_dir().join("quick-translate.json")
}

fn migrate_legacy_bg_layouts(v: &mut serde_json::Value) {
    if v.get("bubbleBgLayouts").or_else(|| v.get("bubble_bg_layouts")).is_some() {
        return;
    }
    let Some(obj) = v.as_object_mut() else {
        return;
    };
    let pos_x = obj
        .get("bubbleBgPosX")
        .or_else(|| obj.get("bubble_bg_pos_x"))
        .and_then(|x| x.as_u64())
        .unwrap_or(50) as u8;
    let pos_y = obj
        .get("bubbleBgPosY")
        .or_else(|| obj.get("bubble_bg_pos_y"))
        .and_then(|x| x.as_u64())
        .unwrap_or(50) as u8;
    let zoom = obj
        .get("bubbleBgZoom")
        .or_else(|| obj.get("bubble_bg_zoom"))
        .and_then(|x| x.as_u64())
        .unwrap_or(100) as u8;
    let layout = serde_json::json!({
        "loading": { "posX": pos_x, "posY": pos_y, "zoom": zoom },
        "success": { "posX": pos_x, "posY": pos_y, "zoom": zoom },
    });
    obj.insert("bubbleBgLayouts".to_string(), layout);
}

pub fn load() -> TranslateSettings {
    let path = config_path();
    if !path.exists() {
        return TranslateSettings::default();
    }
    match fs::read_to_string(&path) {
        Ok(raw) => {
            let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&raw) else {
                return TranslateSettings::default();
            };
            migrate_legacy_bg_layouts(&mut v);
            let s: TranslateSettings = serde_json::from_value(v).unwrap_or_default();
            normalize_settings(s)
        }
        Err(_) => TranslateSettings::default(),
    }
}

pub fn save(settings: &TranslateSettings) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(path, raw).map_err(|e| e.to_string())
}
