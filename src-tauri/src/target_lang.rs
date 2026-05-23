//! 目标语言：支持「自动」——中文原文译英文，非中文原文译简体中文。

/// 将设置中的目标语言解析为实际使用的语言代码（`zh-Hans` / `en` / …）。
pub fn resolve_target_lang(configured: &str, source_text: &str) -> String {
    if configured != "auto" {
        return configured.to_string();
    }
    if is_chinese_source(source_text) {
        "en".to_string()
    } else {
        "zh-Hans".to_string()
    }
}

/// 判断原文是否以中文（汉字）为主。
pub fn is_chinese_source(text: &str) -> bool {
    let mut han = 0u32;
    let mut latin = 0u32;
    let mut other = 0u32;

    for ch in text.chars() {
        if ch.is_whitespace() {
            continue;
        }
        if is_han_char(ch) {
            han += 1;
        } else if ch.is_ascii_alphabetic() {
            latin += 1;
        } else if !ch.is_ascii_punctuation() {
            other += 1;
        }
    }

    let meaningful = han + latin + other;
    if han == 0 || meaningful == 0 {
        return false;
    }
    // 汉字占比足够高，且不少于拉丁字母数量
    han * 2 >= meaningful && han >= latin
}

fn is_han_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{4E00}'..='\u{9FFF}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{F900}'..='\u{FAFF}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_chinese_to_en() {
        assert_eq!(resolve_target_lang("auto", "你好世界"), "en");
        assert!(is_chinese_source("测试一下"));
    }

    #[test]
    fn auto_english_to_zh() {
        assert_eq!(resolve_target_lang("auto", "hello world"), "zh-Hans");
        assert!(!is_chinese_source("hello world"));
    }

    #[test]
    fn fixed_lang_unchanged() {
        assert_eq!(resolve_target_lang("ja", "hello"), "ja");
    }
}
