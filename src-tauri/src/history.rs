use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::translator::TranslateResult;


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRecord {
    pub id: String,
    pub created_at: u64,
    pub source_text: String,
    pub translated_text: Option<String>,
    pub ok: bool,
    pub error: Option<String>,
    pub provider: Option<String>,
    pub from_cache: bool,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct HistoryFile {
    pub records: Vec<HistoryRecord>,
}

pub fn history_path() -> PathBuf {
    crate::app_data::app_data_dir().join("quick-translate-history.json")
}

pub fn list_records() -> Vec<HistoryRecord> {
    load_file().records
}

pub fn clear_records() -> Result<(), String> {
    let path = history_path();
    if path.exists() {
        fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn delete_record(id: &str) -> Result<(), String> {
    let mut file = load_file();
    let before = file.records.len();
    file.records.retain(|r| r.id != id);
    if file.records.len() == before {
        return Err("记录不存在".to_string());
    }
    save_file(&file)
}

pub fn find_record(id: &str) -> Option<HistoryRecord> {
    load_file().records.into_iter().find(|r| r.id == id)
}

/// 最近一条成功的翻译记录（`records[0]` 为最新）。
pub fn latest_success_record() -> Option<HistoryRecord> {
    list_records().into_iter().find(|r| r.ok)
}

fn text_matches_record(text: &str, rec: &HistoryRecord) -> bool {
    if text == rec.source_text.trim() {
        return true;
    }
    if let Some(ref t) = rec.translated_text {
        if text == t.trim() {
            return true;
        }
    }
    false
}

/// 供日志诊断：说明 [is_stale_history_clipboard_reuse] 各条件是否成立。
pub fn stale_history_clipboard_debug(
    guard_sec: u64,
    captured: &str,
    backup_before_copy: Option<&str>,
    clipboard_sequence_changed: bool,
) -> String {
    let cap = captured.trim();
    let backup = backup_before_copy.map(str::trim).unwrap_or("");
    let mut lines = vec![
        format!("guard_sec: {guard_sec}"),
        format!("captured_len: {}", cap.len()),
        format!("backup_len: {}", backup.len()),
        format!("captured_eq_backup: {}", !cap.is_empty() && cap == backup),
        format!("clipboard_sequence_changed: {clipboard_sequence_changed}"),
    ];
    match latest_success_record() {
        None => lines.push("latest_success_record: None".to_string()),
        Some(rec) => {
            let age_ms = now_ms().saturating_sub(rec.created_at);
            lines.push(format!("latest_record_age_ms: {age_ms}"));
            lines.push(format!(
                "latest_record_source_len: {}",
                rec.source_text.trim().len()
            ));
            lines.push(format!(
                "captured_matches_record: {}",
                text_matches_record(cap, &rec)
            ));
            lines.push(format!(
                "age_gt_guard: {}",
                age_ms > guard_sec.saturating_mul(1000)
            ));
        }
    }
    lines.push(format!(
        "would_block: {}",
        is_stale_history_clipboard_reuse(
            guard_sec,
            captured,
            backup_before_copy,
            clipboard_sequence_changed,
        )
    ));
    lines.join("\n")
}

/// 空选时误用上一条翻译留在剪贴板里的内容：该记录已创建超过 `guard_sec` 秒，且本次复制未产生新内容。
/// 若 `clipboard_sequence_changed` 为 true（序号已变），则视为真实复制成功，不拦截。
pub fn is_stale_history_clipboard_reuse(
    guard_sec: u64,
    captured: &str,
    backup_before_copy: Option<&str>,
    clipboard_sequence_changed: bool,
) -> bool {
    if guard_sec == 0 || clipboard_sequence_changed {
        return false;
    }
    let rec = match latest_success_record() {
        Some(r) => r,
        None => return false,
    };
    let age_ms = now_ms().saturating_sub(rec.created_at);
    if age_ms <= guard_sec.saturating_mul(1000) {
        return false;
    }
    let cap = captured.trim();
    if cap.is_empty() || !text_matches_record(cap, &rec) {
        return false;
    }
    let backup = backup_before_copy.map(str::trim).unwrap_or("");
    cap == backup
}

pub fn trim_to_limit(limit: usize) {
    let limit = limit.clamp(20, 500);
    let mut file = load_file();
    if file.records.len() > limit {
        file.records.truncate(limit);
        let _ = save_file(&file);
    }
}

pub fn append_from_translate(source: &str, result: &TranslateResult, max_records: usize) {
    let max_records = max_records.clamp(20, 500);
    let mut file = load_file();
    let record = HistoryRecord {
        id: format!("{}", now_ms()),
        created_at: now_ms(),
        source_text: source.to_string(),
        translated_text: result.translated_text.clone(),
        ok: result.ok,
        error: result.error.clone(),
        provider: result.provider.clone(),
        from_cache: result.from_cache,
        duration_ms: result.duration_ms,
    };
    file.records.insert(0, record);
    file.records.truncate(max_records);
    let _ = save_file(&file);
}

fn load_file() -> HistoryFile {
    let path = history_path();
    if !path.exists() {
        return HistoryFile::default();
    }
    match fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => HistoryFile::default(),
    }
}

fn save_file(file: &HistoryFile) -> Result<(), String> {
    let path = history_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_string_pretty(file).map_err(|e| e.to_string())?;
    fs::write(path, raw).map_err(|e| e.to_string())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
