use std::thread;
use std::time::{Duration, Instant};

const COPY_RETRY_INTERVAL_MS: u64 = 50;

/// 复制后等待剪贴板更新的最长时间；超时且仍与复制前 backup 相同则视为复制失败，绝不回退使用旧剪贴板。
pub const COPY_VERIFY_MAX_SEC: u64 = 5;

pub const MSG_COPY_FAILED_STALE: &str =
    "复制失败：剪贴板未更新，可能未成功复制选中内容或仍在使用复制前的旧内容";

pub fn read_arboard() -> Option<String> {
    arboard::Clipboard::new()
        .ok()?
        .get_text()
        .ok()
        .filter(|t| !t.trim().is_empty())
}

/// Windows：剪贴板每次变更时递增的序号（[GetClipboardSequenceNumber]）；0 表示空剪贴板，仍是有效值。
#[cfg(windows)]
pub fn clipboard_sequence_number() -> Option<u32> {
    use windows::Win32::System::DataExchange::GetClipboardSequenceNumber;
    unsafe { Some(GetClipboardSequenceNumber()) }
}

#[cfg(not(windows))]
pub fn clipboard_sequence_number() -> Option<u32> {
    None
}

/// 是否视为「复制已成功」：有序号时以序号变化为准（允许内容与 backup 相同）；否则要求文本不同。
pub fn clipboard_copy_succeeded(
    backup_seq: Option<u32>,
    current_seq: Option<u32>,
    backup_text: &str,
    captured_text: &str,
) -> bool {
    let captured = captured_text.trim();
    if captured.is_empty() {
        return false;
    }
    if let (Some(b), Some(c)) = (backup_seq, current_seq) {
        return c != b;
    }
    let backup = backup_text.trim();
    backup.is_empty() || captured != backup
}

/// 热键后、复制前 backup 与当前序号是否相同（用于判断是否发生过剪贴板更新）。
pub fn clipboard_sequence_unchanged(backup_seq: Option<u32>, current_seq: Option<u32>) -> bool {
    match (backup_seq, current_seq) {
        (Some(a), Some(b)) => a == b,
        _ => true,
    }
}

/// 在 [copy_started, copy_started + COPY_VERIFY_MAX_SEC] 内轮询剪贴板；
/// 若始终与复制前 backup 相同则返回 [MSG_COPY_FAILED_STALE]，不再回退使用旧剪贴板。
pub fn read_text_after_copy(backup: Option<&str>, copy_started: Instant) -> Result<String, String> {
    let backup_norm = backup.map(str::trim).unwrap_or("");
    let deadline = copy_started + Duration::from_secs(COPY_VERIFY_MAX_SEC);

    while Instant::now() < deadline {
        if let Some(text) = read_arboard() {
            let trimmed = text.trim();
            if !trimmed.is_empty() && (backup_norm.is_empty() || trimmed != backup_norm) {
                return Ok(trimmed.to_string());
            }
        }
        thread::sleep(Duration::from_millis(COPY_RETRY_INTERVAL_MS));
    }

    if !backup_norm.is_empty() {
        if let Some(text) = read_arboard() {
            if text.trim() == backup_norm {
                return Err(MSG_COPY_FAILED_STALE.to_string());
            }
        }
        return Err(format!(
            "复制失败：超过 {} 秒未能从剪贴板读取到新内容",
            COPY_VERIFY_MAX_SEC
        ));
    }

    Err("请先选中要翻译的文字".to_string())
}

pub fn restore_text(backup: Option<&str>) -> bool {
    match backup {
        Some(s) => arboard::Clipboard::new()
            .and_then(|mut c| c.set_text(s))
            .is_ok(),
        None => true,
    }
}
