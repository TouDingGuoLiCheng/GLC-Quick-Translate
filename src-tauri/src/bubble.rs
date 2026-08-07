use crate::appearance;
use crate::selection::TargetHwnd;
use crate::settings::{self, TranslateSettings};
use crate::window_layout::monitor_work_area_near_cursor;
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use crate::window_util;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

const BUBBLE_LABEL: &str = "translate-bubble";
const BUBBLE_WIDTH: u32 = 320;
const BUBBLE_HEIGHT_MIN: u32 = 72;
const BUBBLE_HEIGHT_MAX: u32 = 220;
/// 成功态固定高度：顶栏 + 原文单行区 + 译文滚动区
const BUBBLE_HEIGHT_SUCCESS: u32 = 152;
/// 失败态手动输入：顶栏 + 输入框 + 操作行
const BUBBLE_HEIGHT_MANUAL: u32 = 168;
const GAP: i32 = 2;
const EDGE_MARGIN: i32 = 12;

static BUBBLE_HIDE_GEN: AtomicU64 = AtomicU64::new(0);
/// 同一次翻译流程内固定气泡左上角，避免加载态与结果态位置跳动
static BUBBLE_ANCHOR: Mutex<Option<BubbleAnchor>> = Mutex::new(None);

static BUBBLE_REPLACE_ACCEL: Mutex<Option<String>> = Mutex::new(None);

/// 气泡模式翻译成功后，可供「气泡内替换」热键写回焦点控件
struct BubbleReplaceSession {
    target: TargetHwnd,
    translated: String,
}

static BUBBLE_REPLACE_SESSION: Mutex<Option<BubbleReplaceSession>> = Mutex::new(None);

pub fn set_replace_session(target: TargetHwnd, translated: String) {
    if !target.is_valid() || translated.trim().is_empty() {
        crate::capture_log::append(&format!(
            "=== [bubble-replace-session] {} clear (invalid target or empty text) target_hwnd={} text_len={} ===\n",
            crate::capture_log::timestamp(),
            target.0,
            translated.len(),
        ));
        clear_replace_session();
        return;
    }
    if let Ok(mut guard) = BUBBLE_REPLACE_SESSION.lock() {
        *guard = Some(BubbleReplaceSession {
            target,
            translated: translated.clone(),
        });
        crate::capture_log::append(&format!(
            "=== [bubble-replace-session] {} set target_hwnd={} translated_len={} ===\n",
            crate::capture_log::timestamp(),
            target.0,
            translated.len(),
        ));
    }
}

pub fn clear_replace_session() {
    let had = if let Ok(mut guard) = BUBBLE_REPLACE_SESSION.lock() {
        let had = guard.is_some();
        *guard = None;
        had
    } else {
        false
    };
    if had {
        crate::capture_log::append(&format!(
            "=== [bubble-replace-session] {} cleared ===\n",
            crate::capture_log::timestamp(),
        ));
    }
}

pub fn take_replace_session() -> Option<(TargetHwnd, String)> {
    let mut guard = BUBBLE_REPLACE_SESSION.lock().ok()?;
    guard.take().map(|s| (s.target, s.translated))
}

pub fn has_replace_session() -> bool {
    BUBBLE_REPLACE_SESSION
        .lock()
        .ok()
        .is_some_and(|g| g.is_some())
}

/// 读取当前译文会话（不消费），供气泡内替换使用。
pub fn peek_replace_session() -> Option<(TargetHwnd, String)> {
    let guard = BUBBLE_REPLACE_SESSION.lock().ok()?;
    guard
        .as_ref()
        .map(|s| (s.target, s.translated.clone()))
}

pub fn is_visible(app: &AppHandle) -> bool {
    app.get_webview_window(BUBBLE_LABEL).is_some()
}

#[derive(Clone, Copy)]
struct BubbleAnchor {
    x: i32,
    y: i32,
    area: (i32, i32, i32, i32),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BubblePayload {
    pub phase: String,
    pub source_text: Option<String>,
    pub translated_text: Option<String>,
    pub provider: Option<String>,
    pub from_cache: bool,
    pub error: Option<String>,
    pub theme: String,
    pub opacity: u8,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BubbleEmit {
    phase: String,
    source_text: Option<String>,
    translated_text: Option<String>,
    provider: Option<String>,
    from_cache: bool,
    error: Option<String>,
    theme: String,
    opacity: u8,
    /// CSS 可用的 data:image/... URL
    background_data_url: Option<String>,
    bubble_bg_layouts: settings::BubbleBgLayouts,
    bubble_text_color: String,
    bubble_muted_color: String,
}

impl From<(BubblePayload, &TranslateSettings)> for BubbleEmit {
    fn from((p, s): (BubblePayload, &TranslateSettings)) -> Self {
        Self {
            phase: p.phase,
            source_text: p.source_text,
            translated_text: p.translated_text,
            provider: p.provider,
            from_cache: p.from_cache,
            error: p.error,
            theme: settings::normalize_app_theme(&s.app_theme),
            opacity: settings::clamp_opacity(s.bubble_opacity),
            background_data_url: appearance::bubble_background_data_url(s),
            bubble_bg_layouts: settings::normalize_bg_layouts(s.bubble_bg_layouts.clone()),
            bubble_text_color: s.bubble_text_color.clone(),
            bubble_muted_color: s.bubble_muted_color.clone(),
        }
    }
}

impl BubblePayload {
    pub fn loading(message: &str) -> Self {
        Self {
            phase: "loading".to_string(),
            source_text: Some(message.to_string()),
            translated_text: None,
            provider: None,
            from_cache: false,
            error: None,
            theme: String::new(),
            opacity: 0,
        }
    }

    pub fn error(message: String, source: Option<String>) -> Self {
        Self {
            phase: "error".to_string(),
            source_text: source,
            translated_text: None,
            provider: None,
            from_cache: false,
            error: Some(message),
            theme: String::new(),
            opacity: 0,
        }
    }
}

fn clear_anchor() {
    if let Ok(mut guard) = BUBBLE_ANCHOR.lock() {
        *guard = None;
    }
}

pub fn hide(app: &AppHandle) {
    BUBBLE_HIDE_GEN.fetch_add(1, Ordering::SeqCst);
    let _ = unregister_bubble_replace_hotkey(app);
    clear_anchor();
    clear_replace_session();
    window_util::close_webview_window(app, BUBBLE_LABEL);
}

/// 气泡显示期间临时注册「气泡内替换」热键；关闭气泡后注销。
pub fn register_bubble_replace_hotkey(
    app: &AppHandle,
    settings: &TranslateSettings,
) -> Result<(), String> {
    if !settings.enabled {
        return Ok(());
    }
    let accel = settings.bubble_replace_hotkey.trim();
    if accel.is_empty() {
        return Ok(());
    }
    unregister_bubble_replace_hotkey(app)?;
    let shortcut = accel
        .parse::<tauri_plugin_global_shortcut::Shortcut>()
        .map_err(|e| format!("无效气泡内替换快捷键 {accel}: {e}"))?;
    app.global_shortcut()
        .register(shortcut)
        .map_err(|e| format!("注册气泡内替换快捷键 {accel} 失败: {e}"))?;
    if let Ok(mut guard) = BUBBLE_REPLACE_ACCEL.lock() {
        *guard = Some(accel.to_string());
    }
    crate::capture_log::append(&format!(
        "=== [bubble-replace-hotkey] {} registered accel={accel} ===\n",
        crate::capture_log::timestamp(),
    ));
    Ok(())
}

pub fn unregister_bubble_replace_hotkey(app: &AppHandle) -> Result<(), String> {
    let accel = BUBBLE_REPLACE_ACCEL
        .lock()
        .ok()
        .and_then(|mut g| g.take());
    if let Some(accel) = accel {
        if let Ok(shortcut) = accel.parse::<tauri_plugin_global_shortcut::Shortcut>() {
            let _ = app.global_shortcut().unregister(shortcut);
        }
        crate::capture_log::append(&format!(
            "=== [bubble-replace-hotkey] {} unregistered accel={accel} ===\n",
            crate::capture_log::timestamp(),
        ));
    }
    Ok(())
}

pub fn present(
    app: &AppHandle,
    settings: &TranslateSettings,
    payload: &BubblePayload,
) -> Result<(), String> {
    BUBBLE_HIDE_GEN.fetch_add(1, Ordering::SeqCst);

    let window = window_util::ensure_webview_window(app, BUBBLE_LABEL)?;

    let height = estimate_bubble_height(payload);
    position_bubble(&window, height)?;
    let emit: BubbleEmit = (payload.clone(), settings).into();
    let _ = window.set_always_on_top(true);
    window
        .show()
        .map_err(|e| format!("显示气泡失败: {e}"))?;
    window
        .emit("bubble:update", emit)
        .map_err(|e| format!("推送气泡状态失败: {e}"))?;

    if payload.phase == "success" {
        register_bubble_replace_hotkey(app, settings)?;
    } else {
        let _ = unregister_bubble_replace_hotkey(app);
    }

    Ok(())
}

pub fn schedule_auto_hide(app: AppHandle, seconds: u64) {
    let sec = seconds.max(3);
    let gen = BUBBLE_HIDE_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(sec));
        if BUBBLE_HIDE_GEN.load(Ordering::SeqCst) == gen {
            hide(&app);
        }
    });
}

/// 取消已安排的气泡自动关闭（进入手动输入时调用）。
pub fn cancel_auto_hide() {
    BUBBLE_HIDE_GEN.fetch_add(1, Ordering::SeqCst);
}

/// 进入手动输入态：取消自动关闭并拉高气泡窗口。
pub fn prepare_manual_input(app: &AppHandle) -> Result<(), String> {
    cancel_auto_hide();
    let window = window_util::ensure_webview_window(app, BUBBLE_LABEL)?;
    position_bubble(&window, BUBBLE_HEIGHT_MANUAL)?;
    Ok(())
}

fn estimate_bubble_height(payload: &BubblePayload) -> u32 {
    match payload.phase.as_str() {
        "loading" => 56,
        "error" => {
            let err_len = payload.error.as_ref().map(|s| s.chars().count()).unwrap_or(0);
            // 顶栏已含「手动输入」；至少按两行正文估算，避免高 DPI 下被压扁
            let lines = ((err_len + 25) / 26).max(2).min(5) as u32;
            86 + lines * 20
        }
        "success" => BUBBLE_HEIGHT_SUCCESS,
        _ => BUBBLE_HEIGHT_SUCCESS,
    }
    .clamp(BUBBLE_HEIGHT_MIN, BUBBLE_HEIGHT_MAX)
}

fn compute_anchor(cx: i32, cy: i32, area: (i32, i32, i32, i32)) -> (i32, i32) {
    let (area_x, area_y, area_w, area_h) = area;
    let w = BUBBLE_WIDTH as i32;
    let h = BUBBLE_HEIGHT_SUCCESS as i32;
    let max_x = area_x + area_w as i32 - w - EDGE_MARGIN;
    let max_y = area_y + area_h as i32 - h - EDGE_MARGIN;
    let min_x = area_x + EDGE_MARGIN;
    let min_y = area_y + EDGE_MARGIN;

    // 默认贴近选区右上方；空间不足时再回退到左上/右下。
    let mut x = cx + GAP;
    let mut y = cy - h - GAP;

    if x > max_x {
        x = cx - w - GAP;
    }
    if y < min_y {
        y = cy + GAP;
    }

    x = x.clamp(min_x, max_x);
    y = y.clamp(min_y, max_y);
    (x, y)
}

fn clamp_to_work_area(x: i32, y: i32, height: u32, area: (i32, i32, i32, i32)) -> (i32, i32) {
    let (area_x, area_y, area_w, area_h) = area;
    let w = BUBBLE_WIDTH as i32;
    let h = height as i32;
    let max_x = area_x + area_w as i32 - w - EDGE_MARGIN;
    let max_y = area_y + area_h as i32 - h - EDGE_MARGIN;
    let min_x = area_x + EDGE_MARGIN;
    let min_y = area_y + EDGE_MARGIN;

    let x = x.clamp(min_x, max_x);
    let mut y = y;
    if y + h > area_y + area_h as i32 - EDGE_MARGIN {
        y = area_y + area_h as i32 - h - EDGE_MARGIN;
    }
    y = y.clamp(min_y, max_y);
    (x, y)
}

fn position_bubble(window: &WebviewWindow, height: u32) -> Result<(), String> {
    let anchor = {
        let mut guard = BUBBLE_ANCHOR
            .lock()
            .map_err(|_| "气泡锚点锁失败".to_string())?;
        if let Some(a) = *guard {
            a
        } else {
            let (cx, cy) = cursor_position()?;
            let area = monitor_work_area_near_cursor()?;
            let (x, y) = compute_anchor(cx, cy, area);
            let created = BubbleAnchor { x, y, area };
            *guard = Some(created);
            created
        }
    };

    let (x, y) = clamp_to_work_area(anchor.x, anchor.y, height, anchor.area);

    window
        .set_size(PhysicalSize::new(BUBBLE_WIDTH, height))
        .map_err(|e| format!("设置气泡尺寸失败: {e}"))?;
    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|e| format!("设置气泡位置失败: {e}"))?;
    Ok(())
}

#[cfg(windows)]
fn cursor_position() -> Result<(i32, i32), String> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    unsafe {
        let mut pt = POINT::default();
        GetCursorPos(&mut pt).map_err(|e| format!("GetCursorPos 失败: {e}"))?;
        Ok((pt.x, pt.y))
    }
}

#[cfg(not(windows))]
fn cursor_position() -> Result<(i32, i32), String> {
    Ok((200, 200))
}
