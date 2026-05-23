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
