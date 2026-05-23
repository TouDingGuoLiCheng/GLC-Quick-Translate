use crate::cache::{cache_key, TranslateCache};
use crate::http_translate;
use crate::providers::{self, TranslatorProvider, TranslatorsFile};
use crate::settings::TranslateSettings;
use serde::Serialize;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use crate::window_util;
use tauri::{AppHandle, Manager, Url, WebviewWindow};

const SCRAPER_LABEL: &str = "translate-scraper";
const MAX_TEXT_LEN: usize = 1500;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslateResult {
    pub ok: bool,
    pub source_text: String,
    pub translated_text: Option<String>,
    pub provider: Option<String>,
    pub from_cache: bool,
    pub error: Option<String>,
    pub duration_ms: u64,
}

pub fn translate(
    app: &AppHandle,
    cache: &TranslateCache,
    settings: &TranslateSettings,
    source_text: &str,
) -> TranslateResult {
    let started = Instant::now();
    let text = truncate_text(source_text.trim());
    if text.is_empty() {
        return TranslateResult {
            ok: false,
            source_text: source_text.to_string(),
            translated_text: None,
            provider: None,
            from_cache: false,
            error: Some("原文为空".to_string()),
            duration_ms: started.elapsed().as_millis() as u64,
        };
    }

    let effective_target =
        crate::target_lang::resolve_target_lang(&settings.target_lang, &text);
    let key = cache_key(&text, &effective_target);
    let ttl = Duration::from_secs(settings.cache_ttl_sec.max(5));
    if let Some(hit) = cache.get(&key, ttl) {
        if !crate::http_translate::is_placeholder_translation(&hit.translated) {
            return TranslateResult {
                ok: true,
                source_text: text,
                translated_text: Some(hit.translated),
                provider: Some(hit.provider),
                from_cache: true,
                error: None,
                duration_ms: started.elapsed().as_millis() as u64,
            };
        }
    }

    let file = providers::load_translators();
    let chain = providers::provider_chain(
        &file,
        &settings.primary_provider,
        settings.fallback_enabled,
    );
    if chain.is_empty() {
        return TranslateResult {
            ok: false,
            source_text: text,
            translated_text: None,
            provider: None,
            from_cache: false,
            error: Some("没有可用的翻译源，请检查 translators.json".to_string()),
            duration_ms: started.elapsed().as_millis() as u64,
        };
    }

    let _scraper_guard = ScraperGuard::new(app, settings);

    // 包名/库名（如 pyperclip）：词典与百度网页通常无结果，避免落入 WebView 整段超时
    if http_translate::is_identifier_like(&text) {
        if let Some(translated) =
            http_translate::try_http("baidu", &text, &effective_target)
        {
            cache.set(key.clone(), translated.clone(), "baidu".to_string());
            return TranslateResult {
                ok: true,
                source_text: text,
                translated_text: Some(translated),
                provider: Some("百度翻译".to_string()),
                from_cache: false,
                error: None,
                duration_ms: started.elapsed().as_millis() as u64,
            };
        }
        cache.set(key.clone(), text.clone(), "identifier".to_string());
        return TranslateResult {
            ok: true,
            source_text: text.clone(),
            translated_text: Some(text),
            provider: Some("专有名词（保留原文）".to_string()),
            from_cache: false,
            error: None,
            duration_ms: started.elapsed().as_millis() as u64,
        };
    }

    let deadline = started + Duration::from_secs(settings.timeout_sec.max(3));
    let mut last_err = String::from("翻译超时");

    for provider in chain {
        if Instant::now() >= deadline {
            break;
        }

        // 优先 HTTP（百度 sug / 必应 ttranslatev3），避免 WebView 慢加载导致超时
        if let Some(translated) =
            http_translate::try_http(&provider.id, &text, &effective_target)
        {
            cache.set(key, translated.clone(), provider.id.clone());
            return TranslateResult {
                ok: true,
                source_text: text,
                translated_text: Some(translated),
                provider: Some(provider.name.clone()),
                from_cache: false,
                error: None,
                duration_ms: started.elapsed().as_millis() as u64,
            };
        }

        let scraper = match get_scraper(app, settings) {
            Ok(w) => w,
            Err(e) => {
                last_err = e;
                continue;
            }
        };

        match translate_with_provider(&scraper, provider, &text, &effective_target, deadline) {
            Ok(translated) => {
                cache.set(
                    key,
                    translated.clone(),
                    provider.id.clone(),
                );
                return TranslateResult {
                    ok: true,
                    source_text: text,
                    translated_text: Some(translated),
                    provider: Some(provider.name.clone()),
                    from_cache: false,
                    error: None,
                    duration_ms: started.elapsed().as_millis() as u64,
                };
            }
            Err(e) => last_err = e,
        }
    }

    TranslateResult {
        ok: false,
        source_text: text,
        translated_text: None,
        provider: None,
        from_cache: false,
        error: Some(last_err),
        duration_ms: started.elapsed().as_millis() as u64,
    }
}

struct ScraperGuard<'a> {
    app: &'a AppHandle,
    settings: &'a TranslateSettings,
}

impl<'a> ScraperGuard<'a> {
    fn new(app: &'a AppHandle, settings: &'a TranslateSettings) -> Self {
        Self { app, settings }
    }
}

impl Drop for ScraperGuard<'_> {
    fn drop(&mut self) {
        if self.settings.debug_scraper {
            if let Some(w) = self.app.get_webview_window(SCRAPER_LABEL) {
                let _ = w.hide();
            }
        } else {
            window_util::close_webview_window(self.app, SCRAPER_LABEL);
        }
    }
}

fn get_scraper(app: &AppHandle, settings: &TranslateSettings) -> Result<WebviewWindow, String> {
    let window = window_util::ensure_webview_window(app, SCRAPER_LABEL)?;
    if settings.debug_scraper {
        let _ = window.show();
    } else {
        let _ = window.hide();
    }
    Ok(window)
}

fn translate_with_provider(
    window: &WebviewWindow,
    provider: &TranslatorProvider,
    text: &str,
    target_lang: &str,
    deadline: Instant,
) -> Result<String, String> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    let min_webview = Duration::from_millis(provider.wait_ms.saturating_add(1500));
    if remaining < min_webview {
        return Err(format!(
            "{} WebView 剩余时间不足（{}ms），已跳过",
            provider.name,
            remaining.as_millis()
        ));
    }

    let url = providers::build_url(&provider.url_template, &provider.id, target_lang, text);
    let parsed = Url::parse(&url).map_err(|e| format!("URL 无效: {e}"))?;
    window
        .navigate(parsed)
        .map_err(|e| format!("打开 {} 失败: {e}", provider.name))?;

    thread::sleep(Duration::from_millis(provider.wait_ms));

    let selectors = parse_selectors(&provider.result_selector);
    let selectors_json =
        serde_json::to_string(&selectors).unwrap_or_else(|_| "[]".to_string());

    for attempt in 0..provider.max_poll_attempts {
        if Instant::now() >= deadline {
            return Err(format!("{} 超时", provider.name));
        }

        if provider.id == "baidu" && attempt == 0 {
            let _ = eval_inject_baidu(window, text);
        }
        if provider.id == "bing" && attempt == 0 {
            let _ = eval_inject_bing(window, text);
        }

        let result = match provider.id.as_str() {
            "baidu" => eval_scrape_baidu(window, text)?,
            "bing" => eval_scrape_bing(window, text)?,
            _ => eval_scrape(window, &selectors_json)?,
        };

        if let Some(result) = result {
            if is_valid_translation(text, &result) {
                return Ok(result);
            }
        }

        thread::sleep(Duration::from_millis(provider.poll_interval_ms));
        let _ = attempt;
    }

    Err(format!("{} 未能解析译文（选择器可能已失效）", provider.name))
}

fn parse_selectors(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

/// 百度翻译：注入原文并读取译文区（[百度翻译网页](https://fanyi.baidu.com/mtpe-individual/transText)）
fn eval_inject_baidu(window: &WebviewWindow, source_text: &str) -> Result<(), String> {
    let source_json =
        serde_json::to_string(source_text).unwrap_or_else(|_| "\"\"".to_string());
    let script = format!(
        r#"(function() {{
          const source = {source_json};
          const norm = (s) => (s || '').trim();
          const src = norm(source);
          const areas = [...document.querySelectorAll('textarea')];
          let input = areas.find((t) => {{
            const ph = (t.placeholder || '').trim();
            return ph.includes('输入') || ph.includes('请输入');
          }});
          if (!input) input = areas[0];
          if (!input) return false;
          if (norm(input.value) !== src) {{
            input.value = source;
            input.dispatchEvent(new Event('input', {{ bubbles: true }}));
            input.dispatchEvent(new Event('change', {{ bubbles: true }}));
          }}
          const clickables = [...document.querySelectorAll('button, [role="button"], span, div')];
          const btn = clickables.find((el) => {{
            const t = norm(el.innerText || el.textContent);
            return t === 'AI翻译' || t === '翻译' || t === '开始翻译';
          }});
          if (btn) btn.click();
          return true;
        }})()"#
    );
    let _ = run_eval_script(window, &script)?;
    Ok(())
}

fn eval_scrape_baidu(window: &WebviewWindow, source_text: &str) -> Result<Option<String>, String> {
    let source_json =
        serde_json::to_string(source_text).unwrap_or_else(|_| "\"\"".to_string());
    let script = format!(
        r#"(function() {{
          const source = {source_json};
          const norm = (s) => (s || '').trim();
          const src = norm(source);
          const junk = new Set([
            '请输入文本', '请输入', 'AI翻译', '深度思考', '参考知识', '个性指令',
            '百度翻译', '文本翻译', '划译', '上传图片/文档', '自动检测', '中文(简体)'
          ]);
          const ok = (t) => {{
            if (!t) return false;
            if (t === src) return false;
            if (junk.has(t)) return false;
            if (t.length < 2 && src.length > 6) return false;
            return true;
          }};
          const read = (el) => {{
            if (!el) return null;
            const t = norm(el.innerText || el.textContent || el.value);
            return ok(t) ? t : null;
          }};

          const areas = [...document.querySelectorAll('textarea')];
          if (areas.length >= 2) {{
            for (let i = areas.length - 1; i >= 0; i--) {{
              const ph = (areas[i].placeholder || '');
              if (ph.includes('输入') || ph.includes('请输入')) continue;
              const t = read(areas[i]);
              if (t) return t;
            }}
            const t = read(areas[areas.length - 1]);
            if (t) return t;
          }}

          const candidates = document.querySelectorAll(
            '[class*="target"], [class*="result"], [class*="output"], .markdown-body, [contenteditable="true"]'
          );
          for (const el of candidates) {{
            const t = read(el);
            if (t) return t;
          }}
          return null;
        }})()"#
    );
    run_eval_script(window, &script)
}

/// 必应：写入原文到输入框（[cn.bing 翻译页](https://cn.bing.com/translator?setmkt=zh-cn&setlang=zh-cn)）
fn eval_inject_bing(window: &WebviewWindow, source_text: &str) -> Result<(), String> {
    let source_json =
        serde_json::to_string(source_text).unwrap_or_else(|_| "\"\"".to_string());
    let script = format!(
        r#"(function() {{
          const source = {source_json};
          const norm = (s) => (s || '').trim();
          const src = norm(source);
          if (!src) return;
          const setVal = (el) => {{
            if (!el) return false;
            if (el.value !== undefined) {{
              el.value = src;
              el.dispatchEvent(new Event('input', {{ bubbles: true }}));
              return true;
            }}
            if (el.isContentEditable) {{
              el.textContent = src;
              el.dispatchEvent(new Event('input', {{ bubbles: true }}));
              return true;
            }}
            return false;
          }};
          if (setVal(document.querySelector('#tta_input_ta'))) return;
          const areas = document.querySelectorAll('textarea');
          for (const ta of areas) {{
            const ph = (ta.placeholder || '').toLowerCase();
            if (ph.includes('type') || ph.includes('输入') || ph.includes('enter')) {{
              if (setVal(ta)) return;
            }}
          }}
          if (areas.length > 0) setVal(areas[0]);
        }})()"#
    );
    run_eval_script(window, &script).map(|_| ())
}

/// 必应专用：只读译文区 textarea，排除原文与页面杂项文本（如误抓「超市」等 UI 字）
fn eval_scrape_bing(window: &WebviewWindow, source_text: &str) -> Result<Option<String>, String> {
    let source_json =
        serde_json::to_string(source_text).unwrap_or_else(|_| "\"\"".to_string());
    let script = format!(
        r#"(function() {{
          const source = {source_json};
          const norm = (s) => (s || '').trim();
          const src = norm(source);
          const junk = new Set([
            '超市', '翻译', 'Translating', 'Undo', 'Copy', 'Search',
            'Sorry, something went wrong', 'Try refreshing the page',
            '很抱歉，出现了问题', '请尝试刷新页面', '您已超过允许的翻译次数',
            '...', '…'
          ]);
          const isDots = (t) => /^[.\u2026\s]+$/.test(t);
          const ok = (t) => {{
            if (!t) return false;
            if (t === src) return false;
            if (junk.has(t)) return false;
            if (isDots(t)) return false;
            if (t.length <= 2 && src.length > 8) return false;
            return true;
          }};
          const read = (el) => {{
            if (!el) return null;
            const t = norm(el.value !== undefined ? el.value : (el.innerText || el.textContent));
            return ok(t) ? t : null;
          }};

          let t = read(document.querySelector('#tta_output_ta'));
          if (t) return t;

          const input = document.querySelector('#tta_input_ta');
          const areas = document.querySelectorAll('textarea');
          for (const ta of areas) {{
            if (ta === input) continue;
            t = read(ta);
            if (t) return t;
          }}

          const tgt = document.querySelector('#tgtTrans');
          if (tgt) {{
            const right = tgt.querySelectorAll('textarea, [contenteditable="true"]');
            for (const el of right) {{
              t = read(el);
              if (t) return t;
            }}
          }}

          return null;
        }})()"#
    );
    run_eval_script(window, &script)
}

fn is_valid_translation(source: &str, result: &str) -> bool {
    let s = source.trim();
    let r = result.trim();
    if r.is_empty() || crate::http_translate::is_placeholder_translation(r) {
        return false;
    }
    if r == s {
        return false;
    }
    const JUNK: &[&str] = &[
        "超市",
        "翻译",
        "Translating",
        "Undo",
        "请输入文本",
        "AI翻译",
        "...",
        "…",
    ];
    if JUNK.contains(&r) {
        return false;
    }
    true
}

fn eval_scrape(window: &WebviewWindow, selectors_json: &str) -> Result<Option<String>, String> {
    let script = format!(
        r#"(function() {{
          const selectors = {selectors_json};
          for (const sel of selectors) {{
            const el = document.querySelector(sel);
            if (!el) continue;
            const t = (el.innerText || el.textContent || el.value || '').trim();
            if (t.length > 0) return t;
          }}
          return null;
        }})()"#
    );
    run_eval_script(window, &script)
}

fn run_eval_script(window: &WebviewWindow, script: &str) -> Result<Option<String>, String> {
    let (tx, rx) = mpsc::channel();
    window
        .eval_with_callback(script.to_string(), move |raw| {
            let _ = tx.send(raw);
        })
        .map_err(|e| format!("执行刮取脚本失败: {e}"))?;

    let raw = rx
        .recv_timeout(Duration::from_secs(3))
        .map_err(|_| "刮取脚本回调超时".to_string())?;

    parse_eval_json(&raw)
}

fn parse_eval_json(raw: &str) -> Result<Option<String>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "null" {
        return Ok(None);
    }
    if let Ok(s) = serde_json::from_str::<String>(trimmed) {
        return Ok(if s.trim().is_empty() {
            None
        } else {
            Some(s)
        });
    }
    Ok(Some(trimmed.to_string()))
}

fn truncate_text(text: &str) -> String {
    if text.chars().count() <= MAX_TEXT_LEN {
        return text.to_string();
    }
    text.chars().take(MAX_TEXT_LEN).collect()
}

#[allow(dead_code)]
pub fn list_provider_names(file: &TranslatorsFile) -> Vec<(String, String)> {
    file.providers
        .iter()
        .map(|p| (p.id.clone(), p.name.clone()))
        .collect()
}
