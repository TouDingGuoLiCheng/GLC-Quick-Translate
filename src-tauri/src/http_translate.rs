//! 不经 WebView 的轻量 HTTP 翻译（百度 sug、必应 ttranslatev3、有道/MyMemory 兜底）

use serde::Deserialize;
use std::time::Duration;

const HTTP_TIMEOUT: Duration = Duration::from_secs(6);
const IDENT_HTTP_TIMEOUT: Duration = Duration::from_secs(4);
const UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36 Edg/122.0.0.0";

/// 专有名词 / Python 包名（如 pyperclip）：HTTP 允许返回与原文相同，避免拖入 WebView 超时。
pub fn is_identifier_like(text: &str) -> bool {
    let s = text.trim();
    if s.is_empty() || s.len() > 80 || s.contains(char::is_whitespace) {
        return false;
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    {
        return false;
    }
    if s.contains('_') || s.contains('.') {
        return true;
    }
    if s.chars().any(|c| c.is_ascii_digit()) || s.chars().any(|c| c.is_ascii_uppercase()) {
        return true;
    }
    // 全小写包名：pyperclip、pypercli、requests 等
    s.len() >= 6 && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

/// 若该源支持 HTTP 且成功则返回译文，否则 `None`（由调用方回退 WebView）
pub fn try_http(provider_id: &str, text: &str, target_lang: &str) -> Option<String> {
    match provider_id {
        "baidu" => try_baidu_http_chain(text, target_lang),
        "bing" => try_one_http(text, translate_bing(text, target_lang)),
        _ => None,
    }
}

/// 含空格、较长文本，或 ≥2 个汉字的词组：按整句翻译，不走词典 sug
pub fn is_sentence_like(text: &str) -> bool {
    let t = text.trim();
    if t.contains(char::is_whitespace) || t.chars().count() > 24 {
        return true;
    }
    count_cjk_chars(t) >= 2
}

fn count_cjk_chars(text: &str) -> usize {
    text.chars()
        .filter(|c| ('\u{4e00}'..='\u{9fff}').contains(c))
        .count()
}

fn try_baidu_http_chain(text: &str, target_lang: &str) -> Option<String> {
    if is_sentence_like(text) {
        return try_sentence_http_chain(text, target_lang);
    }
    if let Some(s) = try_one_http(text, translate_baidu_sug(text)) {
        return Some(s);
    }
    // 包名/专有名词：有道常返回 102，优先 MyMemory
    if is_identifier_like(text) {
        if let Some(s) = try_one_http(text, translate_mymemory(text, target_lang, IDENT_HTTP_TIMEOUT)) {
            return Some(s);
        }
        return try_one_http(text, translate_youdao_demo(text, target_lang, IDENT_HTTP_TIMEOUT));
    }
    try_sentence_http_chain(text, target_lang)
}

fn try_sentence_http_chain(text: &str, target_lang: &str) -> Option<String> {
    if let Some(s) = try_one_http(text, translate_youdao_demo(text, target_lang, HTTP_TIMEOUT)) {
        return Some(s);
    }
    try_one_http(text, translate_mymemory(text, target_lang, HTTP_TIMEOUT))
}

fn try_one_http(text: &str, result: Result<String, String>) -> Option<String> {
    match result {
        Ok(s) if is_usable_http(text, &s) => Some(s),
        _ => None,
    }
}

/// 必应/WebView 加载中的占位符，不能当作译文
pub fn is_placeholder_translation(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() {
        return true;
    }
    if t == "..." || t == "…" || t == "...." {
        return true;
    }
    t.chars().all(|c| c == '.' || c == '…' || c.is_whitespace())
}

fn is_usable_http(source: &str, result: &str) -> bool {
    let s = source.trim();
    let r = result.trim();
    if r.is_empty() || is_placeholder_translation(r) {
        return false;
    }
    if r != s {
        return true;
    }
    is_identifier_like(s)
}

fn translate_youdao_demo(
    text: &str,
    target_lang: &str,
    timeout: Duration,
) -> Result<String, String> {
    let to = youdao_to(target_lang);
    let body = format!(
        "q={}&from=Auto&to={to}",
        urlencoding::encode(text)
    );
    let agent = ureq::AgentBuilder::new().timeout(timeout).build();
    let resp = agent
        .post("https://aidemo.youdao.com/trans")
        .set("User-Agent", UA)
        .set("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8")
        .send_string(&body)
        .map_err(|e| format!("有道 HTTP 失败: {e}"))?;
    if !(200..300).contains(&resp.status()) {
        return Err(format!("有道 HTTP {}", resp.status()));
    }
    let raw = read_body_utf8(resp)?;
    parse_youdao_demo(&raw)
}

fn parse_youdao_demo(raw: &str) -> Result<String, String> {
    let v: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| format!("有道 JSON 解析失败: {e}"))?;
    let code = v
        .get("errorCode")
        .map(|c| match c {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            _ => c.to_string(),
        })
        .unwrap_or_default();
    if code != "0" {
        return Err(format!("有道错误码 {code}"));
    }
    let t = v
        .get("translation")
        .and_then(|t| t.as_array())
        .and_then(|arr| arr.first())
        .and_then(|x| x.as_str())
        .ok_or_else(|| "有道未返回译文".to_string())?;
    let trimmed = t.trim();
    if trimmed.is_empty() {
        return Err("有道译文为空".to_string());
    }
    Ok(trimmed.to_string())
}

fn translate_mymemory(
    text: &str,
    target_lang: &str,
    timeout: Duration,
) -> Result<String, String> {
    let pair = mymemory_langpair(target_lang);
    let url = format!(
        "https://api.mymemory.translated.net/get?q={}&langpair={pair}",
        urlencoding::encode(text)
    );
    let body = http_get(&url, timeout)?;
    parse_mymemory(&body)
}

fn parse_mymemory(raw: &str) -> Result<String, String> {
    let v: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| format!("MyMemory 解析失败: {e}"))?;
    let status = v
        .get("responseStatus")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    if status != 200 {
        return Err(format!("MyMemory 状态 {status}"));
    }
    let t = v
        .get("responseData")
        .and_then(|d| d.get("translatedText"))
        .and_then(|x| x.as_str())
        .ok_or_else(|| "MyMemory 无译文".to_string())?;
    let trimmed = t.trim();
    if trimmed.is_empty() {
        return Err("MyMemory 译文为空".to_string());
    }
    Ok(trimmed.to_string())
}

fn mymemory_langpair(target_lang: &str) -> &'static str {
    match target_lang {
        "en" => "zh-CN|en",
        "zh-Hant" => "zh-TW|en",
        "ja" => "zh-CN|ja",
        _ => "en|zh-CN",
    }
}

fn translate_baidu_sug(text: &str) -> Result<String, String> {
    let url = format!(
        "https://fanyi.baidu.com/sug?kw={}&json=1",
        urlencoding::encode(text)
    );
    let body = http_get(&url, HTTP_TIMEOUT)?;
    parse_baidu_sug(&body)
}

fn http_get(url: &str, timeout: Duration) -> Result<String, String> {
    http_get_with_referer(url, timeout, None)
}

fn http_get_with_referer(
    url: &str,
    timeout: Duration,
    referer: Option<&str>,
) -> Result<String, String> {
    let agent = ureq::AgentBuilder::new().timeout(timeout).build();
    let mut req = agent.get(url).set("User-Agent", UA);
    if let Some(r) = referer {
        req = req.set("Referer", r);
    }
    let resp = req.call().map_err(|e| format!("HTTP 请求失败: {e}"))?;
    if !(200..300).contains(&resp.status()) {
        return Err(format!("HTTP {}", resp.status()));
    }
    read_body_utf8(resp)
}

fn read_body_utf8(resp: ureq::Response) -> Result<String, String> {
    use std::io::Read;
    let mut reader = resp.into_reader();
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|e| format!("读取响应失败: {e}"))?;
    Ok(crate::text_encoding::decode_bytes(&bytes))
}

fn youdao_to(target_lang: &str) -> &'static str {
    match target_lang {
        "en" => "en",
        "zh-Hant" => "zh-CHT",
        "ja" => "ja",
        "ko" => "ko",
        _ => "zh-CHS",
    }
}

#[derive(Debug, Deserialize)]
struct BaiduSugResponse {
    errno: i32,
    data: Vec<BaiduSugItem>,
}

#[derive(Debug, Deserialize)]
struct BaiduSugItem {
    #[allow(dead_code)]
    k: String,
    v: String,
}

fn parse_baidu_sug(body: &str) -> Result<String, String> {
    let resp: BaiduSugResponse =
        serde_json::from_str(body).map_err(|e| format!("百度 sug 解析失败: {e}"))?;
    if resp.errno != 0 || resp.data.is_empty() {
        return Err("百度 sug 无结果（多为整句，需 WebView）".to_string());
    }
    let raw = resp.data[0].v.trim();
    if raw.is_empty() {
        return Err("百度 sug 译文为空".to_string());
    }
    Ok(clean_baidu_sug(raw))
}

/// 词典式释义常带分号，取首条可读释义；去掉 [化]、[计] 等领域标签与词性前缀
fn clean_baidu_sug(raw: &str) -> String {
    let first = raw.split(';').next().unwrap_or(raw).trim();
    let first = first.split('；').next().unwrap_or(first).trim();
    let mut s = first.to_string();
    loop {
        let next = strip_baidu_dict_prefix(&s);
        if next == s {
            break;
        }
        s = next;
    }
    s
}

/// 去掉百度 sug 前的领域方括号标签（如 [化]=化学/化工类）和 n./v. 等词性标记
fn strip_baidu_dict_prefix(s: &str) -> String {
    let mut t = s.trim();
    if t.starts_with('[') {
        if let Some(end) = t.find(']') {
            t = t[end + 1..].trim_start();
        }
    }
    for prefix in [
        "int.", "n.", "v.", "adj.", "adv.", "prep.", "conj.", "pron.",
    ] {
        if let Some(rest) = t.strip_prefix(prefix) {
            return rest.trim().to_string();
        }
    }
    t.to_string()
}

struct BingSession {
    subdomain: String,
    ig: String,
    iid: String,
    key: String,
    token: String,
}

fn bing_http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(HTTP_TIMEOUT).build()
}

fn bing_to_lang(target_lang: &str) -> &'static str {
    match target_lang {
        "zh-Hans" => "zh-Hans",
        "zh-Hant" => "zh-Hant",
        "en" => "en",
        "ja" => "ja",
        "ko" => "ko",
        "fr" => "fr",
        "de" => "de",
        "es" => "es",
        "ru" => "ru",
        _ => "zh-Hans",
    }
}

fn bing_translator_url(subdomain: &str) -> String {
    format!(
        "https://{subdomain}.bing.com/translator?setmkt=zh-cn&setlang=zh-cn"
    )
}

fn parse_bing_session_page(body: &str, subdomain: &str) -> Result<BingSession, String> {
    let ig = extract_quoted_after(body, "IG:\"")
        .ok_or_else(|| "必应页面缺少 IG".to_string())?;
    let iid = extract_quoted_after(body, "data-iid=\"")
        .ok_or_else(|| "必应页面缺少 IID".to_string())?;
    let (key, token, token_ttl_ms) = parse_bing_abuse_helper(body)?;
    let _ = token_ttl_ms;
    Ok(BingSession {
        subdomain: subdomain.to_string(),
        ig,
        iid,
        key,
        token,
    })
}

fn extract_quoted_after(haystack: &str, needle: &str) -> Option<String> {
    let start = haystack.find(needle)? + needle.len();
    let rest = &haystack[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn parse_bing_abuse_helper(body: &str) -> Result<(String, String, u64), String> {
    let marker = "params_AbusePreventionHelper";
    let idx = body
        .find(marker)
        .ok_or_else(|| "必应页面缺少 token".to_string())?;
    let slice = &body[idx..];
    let bracket = slice
        .find('[')
        .ok_or_else(|| "必应 token 格式异常".to_string())?;
    let end = slice[bracket..]
        .find(']')
        .ok_or_else(|| "必应 token 格式异常".to_string())?;
    let json = &slice[bracket..=bracket + end];
    let arr: Vec<serde_json::Value> =
        serde_json::from_str(json).map_err(|e| format!("必应 token 解析失败: {e}"))?;
    if arr.len() < 3 {
        return Err("必应 token 字段不足".to_string());
    }
    let key = arr[0]
        .as_i64()
        .or_else(|| arr[0].as_u64().map(|u| u as i64))
        .ok_or_else(|| "必应 key 无效".to_string())?
        .to_string();
    let token = arr[1]
        .as_str()
        .ok_or_else(|| "必应 token 无效".to_string())?
        .to_string();
    let ttl = arr[2]
        .as_u64()
        .or_else(|| arr[2].as_i64().map(|n| n as u64))
        .unwrap_or(3_600_000);
    Ok((key, token, ttl))
}

fn fetch_bing_session_with_agent(agent: &ureq::Agent, subdomain: &str) -> Result<BingSession, String> {
    let page_url = bing_translator_url(subdomain);
    let resp = agent
        .get(&page_url)
        .set("User-Agent", UA)
        .call()
        .map_err(|e| format!("必应页面请求失败: {e}"))?;
    if !(200..300).contains(&resp.status()) {
        return Err(format!("必应页面 HTTP {}", resp.status()));
    }
    let body = read_body_utf8(resp)?;
    parse_bing_session_page(&body, subdomain)
}

fn bing_post_translate(
    agent: &ureq::Agent,
    session: &BingSession,
    text: &str,
    target_lang: &str,
) -> Result<String, String> {
    let to = bing_to_lang(target_lang);
    let referer = bing_translator_url(&session.subdomain);
    let api_url = format!(
        "https://{}.bing.com/ttranslatev3?isVertical=1&IG={}&IID={}",
        session.subdomain,
        urlencoding::encode(&session.ig),
        urlencoding::encode(&session.iid),
    );
    let form = format!(
        "fromLang=auto-detect&text={}&to={}&token={}&key={}&tryFetchingGenderDebiasedTranslations=true",
        urlencoding::encode(text),
        urlencoding::encode(to),
        urlencoding::encode(&session.token),
        session.key,
    );
    let resp = agent
        .post(&api_url)
        .set("User-Agent", UA)
        .set("Referer", &referer)
        .set(
            "Content-Type",
            "application/x-www-form-urlencoded; charset=UTF-8",
        )
        .send_string(&form)
        .map_err(|e| format!("必应翻译 POST 失败: {e}"))?;
    if !(200..300).contains(&resp.status()) {
        return Err(format!("必应翻译 HTTP {}", resp.status()));
    }
    let raw = read_body_utf8(resp)?;
    parse_bing_translate(&raw)
}

fn translate_bing(text: &str, target_lang: &str) -> Result<String, String> {
    let t = text.trim();
    if t.is_empty() {
        return Err("必应：原文为空".to_string());
    }
    if t.chars().count() > 5000 {
        return Err("必应：原文超过 5000 字".to_string());
    }
    // 同一 Agent 先拉页面拿 token，再 POST；Cookie 必须与 token 同会话，否则会 401
    let agent = bing_http_agent();
    let session = fetch_bing_session_with_agent(&agent, "cn").or_else(|e_cn| {
        fetch_bing_session_with_agent(&agent, "www")
            .map_err(|e_www| format!("{e_cn}; {e_www}"))
    })?;
    let translated = bing_post_translate(&agent, &session, t, target_lang)?;
    if is_placeholder_translation(&translated) {
        return Err("必应返回占位符，请重试".to_string());
    }
    Ok(translated)
}

#[derive(Debug, Deserialize)]
struct BingTranslateItem {
    translations: Vec<BingTranslationLine>,
}

#[derive(Debug, Deserialize)]
struct BingTranslationLine {
    text: String,
}

fn parse_bing_translate(raw: &str) -> Result<String, String> {
    if raw.contains("ShowCaptcha") || raw.contains("StatusCode\":401") {
        return Err("必应：请求受限或需验证码".to_string());
    }
    let items: Vec<BingTranslateItem> =
        serde_json::from_str(raw).map_err(|e| format!("必应 JSON 解析失败: {e}"))?;
    let text = items
        .first()
        .and_then(|i| i.translations.first())
        .map(|t| t.text.trim().to_string())
        .filter(|s| !s.is_empty() && !is_placeholder_translation(s))
        .ok_or_else(|| "必应未返回译文".to_string())?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_baidu_prefix() {
        assert_eq!(clean_baidu_sug("int. 喂你好; 你好"), "喂你好");
        assert_eq!(
            clean_baidu_sug("[化] direction for use"),
            "direction for use"
        );
    }

    #[test]
    fn sentence_skips_sug_path() {
        assert!(is_sentence_like("hello world"));
        assert!(is_sentence_like("这是一段需要翻译的完整句子"));
        assert!(is_sentence_like("使用说明"));
        assert!(!is_sentence_like("pyperclip"));
    }

    #[test]
    fn identifier_allows_unchanged_http() {
        assert!(is_identifier_like("pyperclip"));
        assert!(is_identifier_like("pypercli"));
        assert!(is_usable_http("pyperclip", "pyperclip"));
        assert!(!is_usable_http("hello", "hello"));
    }

    #[test]
    fn parse_mymemory_sample() {
        let raw = r#"{"responseStatus":200,"responseData":{"translatedText":"pyperclip"}}"#;
        assert_eq!(parse_mymemory(raw).unwrap(), "pyperclip");
    }

    /// 需联网：`cargo test http_live -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn http_live_baidu_word() {
        let s = translate_baidu_sug("你好").expect("baidu sug");
        assert!(!s.is_empty());
        eprintln!("baidu: {s}");
    }

    #[test]
    fn parse_youdao_sample() {
        let raw = r#"{"errorCode":"0","translation":["你好世界"]}"#;
        assert_eq!(parse_youdao_demo(raw).unwrap(), "你好世界");
    }

    #[test]
    #[ignore]
    fn http_live_bing_word() {
        let s = translate_bing("解析", "en").expect("bing");
        assert_ne!(s, "...");
        assert_ne!(s, "解析");
        eprintln!("bing: {s}");
    }

    #[test]
    #[ignore]
    fn http_live_pyperclip() {
        let got = try_http("baidu", "pyperclip", "zh-Hans").expect("pyperclip chain");
        assert_eq!(got, "pyperclip");
        eprintln!("pyperclip: {got}");
    }

    #[test]
    #[ignore]
    fn http_live_youdao_sentence() {
        let got = translate_youdao_demo("hello world", "zh-Hans", HTTP_TIMEOUT)
            .unwrap_or_else(|e| panic!("youdao failed: {e}"));
        assert!(!got.is_empty());
        eprintln!("youdao: {got}");
        let chain = try_http("baidu", "hello world", "zh-Hans").expect("baidu chain");
        eprintln!("baidu chain: {chain}");
    }
}
