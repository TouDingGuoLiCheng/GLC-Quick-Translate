use tauri::{AppHandle, Manager, Runtime, WebviewWindow};

/// 按 tauri.conf.json 延迟创建 WebView 窗口（`create: false`），降低托盘待机内存
pub fn ensure_webview_window<R: Runtime>(
    app: &AppHandle<R>,
    label: &str,
) -> Result<WebviewWindow<R>, String> {
    if let Some(w) = app.get_webview_window(label) {
        return Ok(w);
    }
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == label)
        .ok_or_else(|| format!("tauri.conf 中缺少窗口配置: {label}"))?;
    tauri::WebviewWindowBuilder::from_config(app, config)
        .map_err(|e| format!("创建窗口 {label} 失败: {e}"))?
        .visible(false)
        .build()
        .map_err(|e| format!("创建窗口 {label} 失败: {e}"))
}

/// 销毁 WebView 窗口（用 `destroy` 而非 `close`，避免 CloseRequested + prevent_close 导致无法释放内存）
pub fn close_webview_window<R: Runtime>(app: &AppHandle<R>, label: &str) {
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.destroy();
    }
}

/// 收回托盘：销毁主窗及可能残留的气泡/抓取 WebView，尽量回到 ~50MB 待机
pub fn release_all_webviews_for_tray<R: Runtime>(app: &AppHandle<R>) {
    for label in ["main", "translate-bubble", "translate-scraper"] {
        close_webview_window(app, label);
    }
}
