use std::thread;
use std::time::Duration;

const COPY_RETRY_ATTEMPTS: u32 = 16;
const COPY_RETRY_INTERVAL_MS: u64 = 50;

pub fn read_arboard() -> Option<String> {
    arboard::Clipboard::new()
        .ok()?
        .get_text()
        .ok()
        .filter(|t| !t.trim().is_empty())
}

pub fn read_text_after_copy(backup: Option<&str>) -> Result<String, String> {
    let backup_norm = backup.map(str::trim).unwrap_or("");

    for attempt in 0..COPY_RETRY_ATTEMPTS {
        if let Some(text) = read_arboard() {
            let trimmed = text.trim();
            if !trimmed.is_empty() && (backup_norm.is_empty() || trimmed != backup_norm) {
                return Ok(trimmed.to_string());
            }
        }
        if attempt + 1 < COPY_RETRY_ATTEMPTS {
            thread::sleep(Duration::from_millis(COPY_RETRY_INTERVAL_MS));
        }
    }

    if let Some(text) = read_arboard() {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    Err("复制后未能从剪贴板读取文字".to_string())
}

pub fn restore_text(backup: Option<&str>) -> bool {
    match backup {
        Some(s) => arboard::Clipboard::new()
            .and_then(|mut c| c.set_text(s))
            .is_ok(),
        None => true,
    }
}
