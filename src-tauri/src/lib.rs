mod app_data;
mod appearance;
mod autostart;
mod capture_log;
mod clipboard_util;
mod bubble;
mod cache;
mod capture;
mod history;
mod http_translate;
mod providers;
mod target_lang;
mod text_encoding;
mod window_layout;
mod window_util;
mod python_capture;
mod selection;
mod settings;
mod translator;

use cache::TranslateCache;
use capture::{
    capture_selected_text, run_capture_translate_and_emit, SelectionResult, TranslateAction,
};
use settings::TranslateSettings;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, State,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

const TRAY_ID: &str = "main-tray";
const WINDOW_LABEL: &str = "main";
const BUBBLE_LABEL: &str = "translate-bubble";

/// 仅托盘菜单「退出」时为 true；关闭设置/气泡窗不应结束主进程
static APP_QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);

struct AppState {
    settings: Mutex<TranslateSettings>,
    cache: Arc<TranslateCache>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let initial_settings = settings::load();
    let setup_settings = initial_settings.clone();
    let cache = Arc::new(TranslateCache::new());

    tauri::Builder::default()
        .manage(AppState {
            settings: Mutex::new(initial_settings.clone()),
            cache: cache.clone(),
        })
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler({
                    let cache = cache.clone();
                    move |app, shortcut, event| {
                        if event.state != ShortcutState::Pressed {
                            return;
                        }
                        let state = app.state::<AppState>();
                        let settings = state
                            .settings
                            .lock()
                            .map(|g| g.clone())
                            .unwrap_or_default();
                        if !settings.enabled {
                            return;
                        }
                        let is_bubble_replace =
                            shortcut_matches(shortcut, &settings.bubble_replace_hotkey);
                        let is_replace =
                            shortcut_matches(shortcut, &settings.replace_hotkey);
                        let is_translate = shortcut_matches(shortcut, &settings.hotkey);

                        // 气泡已有译文时：替换类热键直接写回，禁止再次「取词中」
                        if bubble::is_visible(&app)
                            && bubble::has_replace_session()
                            && (is_bubble_replace || is_replace)
                        {
                            let app = app.clone();
                            std::thread::spawn(move || {
                                let _ = capture::try_replace_from_active_bubble(&app);
                            });
                            return;
                        }
                        if bubble::is_visible(&app) && is_bubble_replace {
                            let app = app.clone();
                            std::thread::spawn(move || {
                                let _ = capture::try_replace_from_active_bubble(&app);
                            });
                            return;
                        }

                        // 必须在同步阶段捕获前台窗口，否则线程启动后焦点已丢失
                        let target = selection::capture_foreground_target();
                        let action = if is_replace {
                            TranslateAction::Replace
                        } else if is_translate {
                            TranslateAction::Bubble
                        } else {
                            return;
                        };
                        let app = app.clone();
                        let job_settings = settings.clone();
                        let job_cache = cache.clone();
                        std::thread::spawn(move || {
                            run_capture_translate_and_emit(
                                app,
                                job_settings,
                                job_cache,
                                target,
                                action,
                            );
                        });
                    }
                })
                .build(),
        )
        .setup(move |app| {
            providers::ensure_external_translators_config();
            setup_tray(app)?;
            register_hotkeys_on_handle(app.handle(), &setup_settings)
                .map_err(|e| tauri::Error::Anyhow(anyhow::anyhow!(e)))?;
            if let Err(e) = autostart::apply(setup_settings.launch_at_startup) {
                eprintln!("开机启动设置失败: {e}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_translate_settings,
            save_translate_settings,
            capture_selection,
            translate_text,
            list_translators,
            dismiss_bubble,
            list_history,
            clear_history,
            delete_history_record,
            show_history_in_bubble,
            get_python_capture_log_paths,
            open_python_capture_logs,
            clear_python_capture_logs,
            pick_bubble_background,
            clear_bubble_background,
            get_bubble_background_path,
            get_bubble_background_data_url,
            read_image_data_url,
            collapse_main_to_tray
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                match window.label() {
                    WINDOW_LABEL => {
                        window_util::release_all_webviews_for_tray(window.app_handle());
                    }
                    BUBBLE_LABEL => bubble::hide(window.app_handle()),
                    _ => {}
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while running quick-translate")
        .run(|_app_handle, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if !APP_QUIT_REQUESTED.load(Ordering::SeqCst) {
                    api.prevent_exit();
                }
            }
        });
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    if app.tray_by_id(TRAY_ID).is_some() {
        return Ok(());
    }

    let show_i = MenuItem::with_id(app, "show", "打开设置", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

    let icon = app
        .default_window_icon()
        .ok_or_else(|| tauri::Error::FailedToReceiveMessage)?
        .clone();

    let _tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("GLC Quick Translate")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "quit" => {
                APP_QUIT_REQUESTED.store(true, Ordering::SeqCst);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                show_main_window(&app);
            }
        })
        .build(app)?;

    Ok(())
}

fn show_main_window(app: &tauri::AppHandle) {
    let Ok(window) = window_util::ensure_webview_window(app, WINDOW_LABEL) else {
        return;
    };
    let _ = window_layout::place_main_bottom_right(&window);
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

/// 隐藏到托盘：销毁全部 WebView，释放 Chromium 内存
#[tauri::command]
fn collapse_main_to_tray(app: tauri::AppHandle) -> Result<(), String> {
    window_util::release_all_webviews_for_tray(&app);
    Ok(())
}

fn shortcut_matches(
    pressed: &tauri_plugin_global_shortcut::Shortcut,
    accelerator: &str,
) -> bool {
    accelerator
        .parse::<tauri_plugin_global_shortcut::Shortcut>()
        .ok()
        .as_ref()
        == Some(pressed)
}

fn register_hotkeys_on_handle(app: &tauri::AppHandle, settings: &TranslateSettings) -> Result<(), String> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| format!("注销快捷键失败: {e}"))?;
    if !settings.enabled {
        return Ok(());
    }
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    for accel in [
        &settings.hotkey,
        &settings.replace_hotkey,
    ] {
        if !seen.insert(accel.clone()) {
            continue;
        }
        let shortcut = accel
            .parse::<tauri_plugin_global_shortcut::Shortcut>()
            .map_err(|e| format!("无效快捷键 {accel}: {e}"))?;
        app.global_shortcut()
            .register(shortcut)
            .map_err(|e| format!("注册快捷键 {accel} 失败: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
fn get_translate_settings(state: State<'_, AppState>) -> TranslateSettings {
    state
        .settings
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default()
}

#[tauri::command]
fn save_translate_settings(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    mut settings: TranslateSettings,
) -> Result<(), String> {
    settings = settings::normalize_settings(settings);
    settings::save(&settings)?;
    history::trim_to_limit(settings.history_max_count as usize);
    autostart::apply(settings.launch_at_startup)?;
    if let Ok(mut guard) = state.settings.lock() {
        *guard = settings.clone();
    }
    register_hotkeys_on_handle(&app, &settings)?;

    if !settings.debug_scraper {
        window_util::close_webview_window(&app, "translate-scraper");
    } else if let Some(scraper) = app.get_webview_window("translate-scraper") {
        let _ = scraper.show();
    }

    let _ = app.emit("settings:updated", settings);
    Ok(())
}

#[tauri::command]
fn capture_selection(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<SelectionResult, String> {
    let settings = state
        .settings
        .lock()
        .map(|g| g.clone())
        .map_err(|e| e.to_string())?;
    let target = selection::capture_foreground_target();
    Ok(capture_selected_text(&app, &settings, target))
}

#[tauri::command]
fn translate_text(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    text: String,
) -> Result<translator::TranslateResult, String> {
    let settings = state
        .settings
        .lock()
        .map(|g| g.clone())
        .map_err(|e| e.to_string())?;

    let _ = bubble::present(&app, &settings, &bubble::BubblePayload::loading("翻译中…"));

    let result = translator::translate(&app, state.cache.as_ref(), &settings, &text);

    let source = text.clone();
    if result.ok {
        let _ = bubble::present(
            &app,
            &settings,
            &bubble::BubblePayload {
                phase: "success".to_string(),
                source_text: Some(source.clone()),
                translated_text: result.translated_text.clone(),
                provider: result.provider.clone(),
                from_cache: result.from_cache,
                error: None,
                theme: String::new(),
                opacity: 0,
            },
        );
    } else {
        let err = result
            .error
            .clone()
            .unwrap_or_else(|| "翻译失败".to_string());
        let _ = bubble::present(
            &app,
            &settings,
            &bubble::BubblePayload::error(err, Some(source.clone())),
        );
    }

    bubble::schedule_auto_hide(app.clone(), settings.bubble_auto_close_sec);
    record_history_and_emit(&app, &source, &result);
    Ok(result)
}

fn record_history_and_emit(app: &tauri::AppHandle, source: &str, result: &translator::TranslateResult) {
    let max = app
        .try_state::<AppState>()
        .and_then(|s| s.settings.lock().ok().map(|g| g.history_max_count as usize))
        .unwrap_or(200);
    history::append_from_translate(source, result, max);
    let _ = app.emit("history:updated", history::list_records());
}

#[tauri::command]
fn list_history() -> Vec<history::HistoryRecord> {
    history::list_records()
}

#[tauri::command]
fn clear_history(app: tauri::AppHandle) -> Result<(), String> {
    history::clear_records()?;
    let _ = app.emit("history:updated", Vec::<history::HistoryRecord>::new());
    Ok(())
}

#[tauri::command]
fn delete_history_record(app: tauri::AppHandle, id: String) -> Result<(), String> {
    history::delete_record(&id)?;
    let _ = app.emit("history:updated", history::list_records());
    Ok(())
}

#[tauri::command]
fn show_history_in_bubble(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let record = history::find_record(&id).ok_or_else(|| "记录不存在".to_string())?;
    let settings = state
        .settings
        .lock()
        .map(|g| g.clone())
        .map_err(|e| e.to_string())?;

    if record.ok {
        let payload = bubble::BubblePayload {
            phase: "success".to_string(),
            source_text: Some(record.source_text.clone()),
            translated_text: record.translated_text.clone(),
            provider: record.provider.clone(),
            from_cache: record.from_cache,
            error: None,
            theme: String::new(),
            opacity: 0,
        };
        bubble::present(&app, &settings, &payload)?;
    } else {
        let err = record
            .error
            .clone()
            .unwrap_or_else(|| "翻译失败".to_string());
        bubble::present(
            &app,
            &settings,
            &bubble::BubblePayload::error(err, Some(record.source_text.clone())),
        )?;
    }
    bubble::schedule_auto_hide(app, settings.bubble_auto_close_sec);
    Ok(())
}

#[tauri::command]
fn list_translators() -> providers::TranslatorsFile {
    providers::load_translators()
}

#[tauri::command]
fn dismiss_bubble(app: tauri::AppHandle) {
    bubble::hide(&app);
}

#[tauri::command]
fn get_python_capture_log_paths() -> Vec<String> {
    capture_log::paths_for_ui()
}

#[tauri::command]
fn open_python_capture_logs() -> Result<String, String> {
    capture_log::open_logs_folder()
}

#[tauri::command]
fn clear_python_capture_logs() -> Result<usize, String> {
    capture_log::clear_all_logs()
}

#[tauri::command]
fn pick_bubble_background() -> Result<appearance::PickBackgroundResult, String> {
    appearance::pick_and_save_bubble_background()
}

#[tauri::command]
fn clear_bubble_background() -> Result<(), String> {
    appearance::clear_bubble_background_files()
}

#[tauri::command]
fn get_bubble_background_path(state: State<'_, AppState>) -> Option<String> {
    let settings = state.settings.lock().ok()?.clone();
    appearance::bubble_background_display_path(&settings)
}

#[tauri::command]
fn get_bubble_background_data_url(state: State<'_, AppState>) -> Option<String> {
    let settings = state.settings.lock().ok()?.clone();
    appearance::bubble_background_data_url(&settings)
}

#[tauri::command]
fn read_image_data_url(path: String) -> Option<String> {
    appearance::path_to_data_url(std::path::Path::new(&path))
}
