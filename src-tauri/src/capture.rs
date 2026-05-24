use crate::bubble::{self, BubblePayload};
use crate::cache::TranslateCache;
use crate::python_capture;
use crate::selection;
use crate::settings::TranslateSettings;
use crate::translator::{self, TranslateResult};
use serde::Serialize;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tauri_plugin_clipboard_manager::ClipboardExt;

/// 未选中可复制内容时的统一提示（不进入翻译）
const MSG_NO_SELECTION: &str = "请先选中要翻译的文字";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionResult {
    pub ok: bool,
    pub text: Option<String>,
    pub error: Option<String>,
    pub restored_clipboard: bool,
    pub duration_ms: u64,
    /// 本次取词中剪贴板序号已变化（Windows），表示复制确实发生，可与 backup 同文
    #[serde(default)]
    pub clipboard_sequence_changed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslateDonePayload {
    pub selection: SelectionResult,
    pub translate: TranslateResult,
}

pub fn capture_selected_text(
    app: &AppHandle,
    settings: &TranslateSettings,
    target: selection::TargetHwnd,
) -> SelectionResult {
    if !target.is_valid() {
        return SelectionResult {
            ok: false,
            text: None,
            error: Some("未找到目标窗口，请先聚焦到要翻译的应用".to_string()),
            restored_clipboard: false,
            clipboard_sequence_changed: false,
            duration_ms: 0,
        };
    }
    // Windows 统一走 Python 取词（pyautogui），避免 Rust 复制 + python.exe 弹终端
    #[cfg(windows)]
    {
        let _ = app;
        return python_capture::capture_via_python(settings, target);
    }
    #[cfg(not(windows))]
    {
        if settings.use_python_capture {
            python_capture::capture_via_python(settings, target)
        } else {
            capture_selected_text_rust(app, settings, target)
        }
    }
}

fn capture_selected_text_rust(
    app: &AppHandle,
    settings: &TranslateSettings,
    target: selection::TargetHwnd,
) -> SelectionResult {
    let started = std::time::Instant::now();

    let backup = crate::clipboard_util::read_arboard();

    let copy_started = std::time::Instant::now();
    if let Err(e) = selection::simulate_copy_to(target) {
        crate::capture_log::append(&format!(
            "\n=== [capture-copy] {} simulate_copy failed ===\n{e}\n",
            capture_log_timestamp()
        ));
        return SelectionResult {
            ok: false,
            text: None,
            error: Some(format!("复制失败：{e}")),
            restored_clipboard: false,
            clipboard_sequence_changed: false,
            duration_ms: started.elapsed().as_millis() as u64,
        };
    }

    thread::sleep(Duration::from_millis(settings.copy_delay_ms.max(80)));
    let captured = match crate::clipboard_util::read_text_after_copy(backup.as_deref(), copy_started)
    {
        Ok(t) => t,
        Err(e) => {
            log_copy_failure(&e, copy_started);
            let restored =
                settings.restore_clipboard && restore_clipboard(app, backup.as_deref());
            return SelectionResult {
                ok: false,
                text: None,
                error: Some(normalize_copy_error(&e)),
                restored_clipboard: restored,
                clipboard_sequence_changed: false,
                duration_ms: started.elapsed().as_millis() as u64,
            };
        }
    };

    let trimmed = captured.trim();
    if trimmed.is_empty() {
        let restored = settings.restore_clipboard && restore_clipboard(app, backup.as_deref());
        return SelectionResult {
            ok: false,
            text: None,
            error: Some(MSG_NO_SELECTION.to_string()),
            restored_clipboard: restored,
            clipboard_sequence_changed: false,
            duration_ms: started.elapsed().as_millis() as u64,
        };
    }

    let text = trimmed.to_string();
    let restored = settings.restore_clipboard && restore_clipboard(app, backup.as_deref());

    SelectionResult {
        ok: true,
        text: Some(text),
        error: None,
        restored_clipboard: restored,
        clipboard_sequence_changed: false,
        duration_ms: started.elapsed().as_millis() as u64,
    }
}

fn normalize_copy_error(msg: &str) -> String {
    if msg.starts_with("复制失败") {
        return msg.to_string();
    }
    if msg.contains("复制后未能从剪贴板") {
        return crate::clipboard_util::MSG_COPY_FAILED_STALE.to_string();
    }
    format!("复制失败：{msg}")
}

fn log_copy_failure(reason: &str, copy_started: std::time::Instant) {
    let elapsed_ms = copy_started.elapsed().as_millis();
    crate::capture_log::append(&format!(
        "\n=== [capture-copy] {} failed after {elapsed_ms}ms ===\n{reason}\n",
        capture_log_timestamp()
    ));
}

fn capture_log_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("unix={secs}")
}

fn read_clipboard_text(app: &AppHandle) -> Result<String, String> {
    match app.clipboard().read_text() {
        Ok(t) => Ok(t),
        Err(_) => crate::clipboard_util::read_arboard()
            .ok_or_else(|| "剪贴板暂无文本或格式不支持".to_string()),
    }
}

pub fn loading_bubble_payload(settings: &TranslateSettings, hint: &str) -> BubblePayload {
    BubblePayload {
        phase: "loading".to_string(),
        source_text: Some(hint.to_string()),
        translated_text: None,
        provider: Some(settings.primary_provider.clone()),
        from_cache: false,
        error: None,
        theme: String::new(),
        opacity: 0,
    }
}

fn restore_clipboard(app: &AppHandle, backup: Option<&str>) -> bool {
    match backup {
        Some(s) => app.clipboard().write_text(s).is_ok(),
        None => app.clipboard().clear().is_ok(),
    }
}

fn bubble_success_payload(source: &str, translate: &TranslateResult) -> BubblePayload {
    let display_source = if source.trim().is_empty() {
        translate.source_text.clone()
    } else {
        source.to_string()
    };
    BubblePayload {
        phase: "success".to_string(),
        source_text: Some(display_source),
        translated_text: translate.translated_text.clone(),
        provider: translate.provider.clone(),
        from_cache: translate.from_cache,
        error: None,
        theme: String::new(),
        opacity: 0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslateAction {
    /// 显示翻译气泡
    Bubble,
    /// 用译文替换选中文字（不显示气泡）
    Replace,
}

pub fn try_replace_from_active_bubble(app: &AppHandle) -> Result<(), String> {
    if !bubble::is_visible(app) {
        return Ok(());
    }
    let Some((target, translated)) = bubble::peek_replace_session() else {
        let settings = crate::settings::load();
        let msg = "暂无译文可替换，请先完成翻译".to_string();
        let _ = bubble::present(app, &settings, &BubblePayload::error(msg.clone(), None));
        bubble::schedule_auto_hide(app.clone(), settings.bubble_auto_close_sec);
        return Err(msg);
    };

    if !selection::is_target_window_valid(target) {
        let settings = crate::settings::load();
        let msg = "目标窗口已关闭，请重新选中文本后再翻译".to_string();
        let _ = bubble::present(app, &settings, &BubblePayload::error(msg.clone(), None));
        bubble::schedule_auto_hide(app.clone(), settings.bubble_auto_close_sec);
        return Err(msg);
    }

    write_clipboard_text(app, &translated)?;
    bubble::hide(app);
    thread::sleep(Duration::from_millis(120));

    match selection::simulate_paste_to(target) {
        Ok(()) => {
            bubble::clear_replace_session();
            Ok(())
        }
        Err(e) => {
            let settings = crate::settings::load();
            let _ = bubble::present(
                app,
                &settings,
                &BubblePayload::error(format!("替换失败: {e}"), None),
            );
            bubble::schedule_auto_hide(app.clone(), settings.bubble_auto_close_sec);
            Err(e)
        }
    }
}

pub fn run_capture_translate_and_emit(
    app: AppHandle,
    settings: TranslateSettings,
    cache: Arc<TranslateCache>,
    target: selection::TargetHwnd,
    action: TranslateAction,
) {
    if action == TranslateAction::Bubble {
        bubble::clear_replace_session();
    }
    if action == TranslateAction::Bubble || action == TranslateAction::Replace {
        let _ = bubble::present(&app, &settings, &loading_bubble_payload(&settings, "取词中…"));
    }

    thread::spawn(move || {
        let _ = app.emit("translate:started", ());

        let mut capture_settings = settings.clone();
        if action == TranslateAction::Replace {
            capture_settings.restore_clipboard = false;
            // 替换热键后部分应用更新剪贴板较慢
            capture_settings.copy_delay_ms = capture_settings.copy_delay_ms.max(300);
        }

        let should_restore_clipboard = settings.restore_clipboard && action != TranslateAction::Replace;
        let hotkey_clipboard = read_clipboard_text(&app).ok();
        let clipboard_backup = if should_restore_clipboard {
            hotkey_clipboard.clone()
        } else {
            None
        };

        let selection = capture_selected_text(&app, &capture_settings, target);

        if !selection.ok {
            let mut msg = selection
                .error
                .clone()
                .unwrap_or_else(|| MSG_NO_SELECTION.to_string());
            if let Some(ref clip) = hotkey_clipboard {
                let clip_norm = clip.trim();
                if !clip_norm.is_empty()
                    && crate::history::is_stale_history_clipboard_reuse(
                        settings.translate_clipboard_guard_sec,
                        clip_norm,
                        hotkey_clipboard.as_deref(),
                        false,
                    )
                {
                    msg = MSG_NO_SELECTION.to_string();
                }
            }
            present_capture_failure(&app, &settings, action, &msg, selection.text.clone());
            let payload = TranslateDonePayload {
                selection,
                translate: TranslateResult {
                    ok: false,
                    source_text: String::new(),
                    translated_text: None,
                    provider: None,
                    from_cache: false,
                    error: Some("取词失败".to_string()),
                    duration_ms: 0,
                },
            };
            let _ = app.emit("translate:done", &payload);
            return;
        }

        let source = selection.text.clone().unwrap_or_default();
        let guard_debug = crate::history::stale_history_clipboard_debug(
            settings.translate_clipboard_guard_sec,
            source.trim(),
            hotkey_clipboard.as_deref(),
            selection.clipboard_sequence_changed,
        );
        if crate::history::is_stale_history_clipboard_reuse(
            settings.translate_clipboard_guard_sec,
            source.trim(),
            hotkey_clipboard.as_deref(),
            selection.clipboard_sequence_changed,
        ) {
            crate::capture_log::append(&format!(
                "\n=== [capture-pipeline:capture-thread] {} ===\nselection_ok: true\ncaptured_len: {}\n{guard_debug}\nfinal_result: REJECT history-guard → 请先选中（未调用翻译）\n",
                capture_log_timestamp(),
                source.trim().len(),
            ));
            present_capture_failure(
                &app,
                &settings,
                action,
                MSG_NO_SELECTION,
                Some(source.clone()),
            );
            let payload = TranslateDonePayload {
                selection: SelectionResult {
                    ok: false,
                    text: None,
                    error: Some(MSG_NO_SELECTION.to_string()),
                    restored_clipboard: selection.restored_clipboard,
                    clipboard_sequence_changed: selection.clipboard_sequence_changed,
                    duration_ms: selection.duration_ms,
                },
                translate: TranslateResult {
                    ok: false,
                    source_text: String::new(),
                    translated_text: None,
                    provider: None,
                    from_cache: false,
                    error: Some("取词失败".to_string()),
                    duration_ms: 0,
                },
            };
            let _ = app.emit("translate:done", &payload);
            return;
        }
        if source.trim().is_empty() {
            present_capture_failure(
                &app,
                &settings,
                action,
                MSG_NO_SELECTION,
                selection.text.clone(),
            );
            let payload = TranslateDonePayload {
                selection: SelectionResult {
                    ok: false,
                    text: None,
                    error: Some(MSG_NO_SELECTION.to_string()),
                    restored_clipboard: selection.restored_clipboard,
                    clipboard_sequence_changed: selection.clipboard_sequence_changed,
                    duration_ms: selection.duration_ms,
                },
                translate: TranslateResult {
                    ok: false,
                    source_text: String::new(),
                    translated_text: None,
                    provider: None,
                    from_cache: false,
                    error: Some("取词失败".to_string()),
                    duration_ms: 0,
                },
            };
            let _ = app.emit("translate:done", &payload);
            return;
        }
        crate::capture_log::append(&format!(
            "\n=== [capture-pipeline:capture-thread] {} ===\nselection_ok: true\ncaptured_len: {}\n{guard_debug}\nfinal_result: PASS → 开始翻译\n",
            capture_log_timestamp(),
            source.trim().len(),
        ));

        if action == TranslateAction::Bubble || action == TranslateAction::Replace {
            let mut loading = loading_bubble_payload(&settings, "翻译中…");
            loading.source_text = Some(truncate_preview(&source));
            let _ = bubble::present(&app, &settings, &loading);
        }

        let translate = translator::translate(&app, cache.as_ref(), &settings, &source);

        crate::capture_log::append(&format!(
            "\n=== [capture-pipeline:translate-done] {} ===\ntranslate_ok: {}\nfrom_cache: {}\nduration_ms: {}\nerror: {}\n",
            capture_log_timestamp(),
            translate.ok,
            translate.from_cache,
            translate.duration_ms,
            translate
                .error
                .as_deref()
                .unwrap_or(""),
        ));

        if action == TranslateAction::Replace {
            if translate.ok {
                if let Some(ref translated) = translate.translated_text {
                    match apply_replace(&app, target, translated) {
                        Ok(()) => bubble::hide(&app),
                        Err(e) => {
                            let _ = bubble::present(
                                &app,
                                &settings,
                                &BubblePayload::error(
                                    format!("替换失败: {e}"),
                                    Some(source.clone()),
                                ),
                            );
                            bubble::schedule_auto_hide(
                                app.clone(),
                                settings.bubble_auto_close_sec,
                            );
                        }
                    }
                } else {
                    bubble::hide(&app);
                }
            } else {
                let err = translate
                    .error
                    .clone()
                    .unwrap_or_else(|| "翻译失败".to_string());
                let _ = bubble::present(
                    &app,
                    &settings,
                    &BubblePayload::error(err, Some(source.clone())),
                );
                bubble::schedule_auto_hide(app.clone(), settings.bubble_auto_close_sec);
            }
        } else if translate.ok {
            if let Some(ref translated) = translate.translated_text {
                bubble::set_replace_session(target, translated.clone());
            }
            let _ = bubble::present(
                &app,
                &settings,
                &bubble_success_payload(&source, &translate),
            );
            bubble::schedule_auto_hide(app.clone(), settings.bubble_auto_close_sec);
        } else {
            let err = translate
                .error
                .clone()
                .unwrap_or_else(|| "翻译失败".to_string());
            let _ = bubble::present(
                &app,
                &settings,
                &BubblePayload::error(err, Some(source.clone())),
            );
            if !translate.source_text.is_empty() {
                let file = crate::providers::load_translators();
                if let Some(primary) = file
                    .providers
                    .iter()
                    .find(|p| p.id == settings.primary_provider)
                {
                    let url = crate::providers::build_url(
                        &primary.url_template,
                        &primary.id,
                        &settings.target_lang,
                        &translate.source_text,
                    );
                    let _ = app.emit("translate:open_browser", url);
                }
            }
            bubble::schedule_auto_hide(app.clone(), settings.bubble_auto_close_sec);
        }

        if should_restore_clipboard {
            let _ = restore_clipboard(&app, clipboard_backup.as_deref());
        }

        crate::history::append_from_translate(
            &source,
            &translate,
            settings.history_max_count as usize,
        );
        let _ = app.emit("history:updated", crate::history::list_records());

        let payload = TranslateDonePayload {
            selection,
            translate,
        };
        let _ = app.emit("translate:done", &payload);
    });
}

fn apply_replace(app: &AppHandle, target: selection::TargetHwnd, text: &str) -> Result<(), String> {
    write_clipboard_text(app, text)?;
    selection::simulate_paste_to(target)
}

fn write_clipboard_text(app: &AppHandle, text: &str) -> Result<(), String> {
    if app.clipboard().write_text(text).is_ok() {
        return Ok(());
    }
    arboard::Clipboard::new()
        .map_err(|e| format!("剪贴板不可用: {e}"))?
        .set_text(text)
        .map_err(|e| format!("写入剪贴板失败: {e}"))
}

fn is_no_selection_message(msg: &str) -> bool {
    let m = msg.trim();
    (m.contains("请先选中")
        || m.contains("无文本")
        || m.contains("未选中")
        || m.contains("剪贴板无")
        || m.contains("empty-clipboard"))
        && !m.contains("复制失败")
}

fn is_copy_failure_message(msg: &str) -> bool {
    msg.trim().contains("复制失败")
}

fn present_capture_failure(
    app: &AppHandle,
    settings: &TranslateSettings,
    action: TranslateAction,
    msg: &str,
    source_hint: Option<String>,
) {
    if action == TranslateAction::Replace
        && is_no_selection_message(msg)
        && !is_copy_failure_message(msg)
    {
        bubble::hide(app);
        return;
    }
    if action == TranslateAction::Bubble || action == TranslateAction::Replace {
        let _ = bubble::present(
            app,
            settings,
            &BubblePayload::error(msg.to_string(), source_hint),
        );
        bubble::schedule_auto_hide(app.clone(), settings.bubble_auto_close_sec);
    }
}

fn truncate_preview(text: &str) -> String {
    const MAX: usize = 80;
    if text.chars().count() <= MAX {
        return text.to_string();
    }
    text.chars().take(MAX).collect::<String>() + "…"
}
