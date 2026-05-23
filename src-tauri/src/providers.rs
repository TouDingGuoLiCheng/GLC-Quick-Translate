use serde::{Deserialize, Serialize};
use std::fs;

const EMBEDDED: &str = include_str!("../resources/translators.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslatorsFile {
    pub version: u32,
    pub providers: Vec<TranslatorProvider>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslatorProvider {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub url_template: String,
    pub result_selector: String,
    pub wait_ms: u64,
    pub poll_interval_ms: u64,
    pub max_poll_attempts: u32,
}

pub fn load_translators() -> TranslatorsFile {
    let mut file = if let Ok(path) = external_translators_path() {
        if path.exists() {
            if let Ok(raw) = fs::read_to_string(&path) {
                if let Ok(f) = serde_json::from_str::<TranslatorsFile>(&raw) {
                    f
                } else {
                    default_translators_file()
                }
            } else {
                default_translators_file()
            }
        } else {
            default_translators_file()
        }
    } else {
        default_translators_file()
    };
    file.providers.retain(|p| p.id != "google");
    file
}

fn default_translators_file() -> TranslatorsFile {
    serde_json::from_str(EMBEDDED).unwrap_or_else(|_| TranslatorsFile {
        version: 1,
        providers: vec![],
    })
}

pub fn ensure_external_translators_config() {
    if let Ok(path) = external_translators_path() {
        if path.exists() {
            if let Ok(raw) = fs::read_to_string(&path) {
                if let Ok(file) = serde_json::from_str::<TranslatorsFile>(&raw) {
                    if file.version >= 4 {
                        return;
                    }
                }
            }
            // 旧版配置缺少百度翻译，用内置新版覆盖
        }
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, EMBEDDED);
    }
}

pub fn external_translators_path() -> Result<std::path::PathBuf, String> {
    Ok(crate::app_data::app_data_dir().join("translators.json"))
}

/// 返回 (from, to) 供 URL 模板使用
pub fn lang_codes(target_lang: &str, provider_id: &str) -> (String, String) {
    let to = match (target_lang, provider_id) {
        ("zh-Hans", "bing") => "zh-Hans",
        ("zh-Hant", "bing") => "zh-Hant",
        ("en", "bing") => "en",
        ("ja", "bing") => "ja",
        (_, "bing") => "zh-Hans",
        _ => target_lang,
    };
    ("auto".to_string(), to.to_string())
}

pub fn build_url(template: &str, provider_id: &str, target_lang: &str, text: &str) -> String {
    if provider_id == "baidu" || provider_id == "bing" {
        let _ = (target_lang, text);
        return template.to_string();
    }
    let (from, to) = lang_codes(target_lang, provider_id);
    let encoded = urlencoding::encode(text);
    template
        .replace("{from}", &from)
        .replace("{to}", &to)
        .replace("{text}", encoded.as_ref())
}

pub fn provider_chain<'a>(
    file: &'a TranslatorsFile,
    primary_id: &str,
    fallback_enabled: bool,
) -> Vec<&'a TranslatorProvider> {
    let mut chain = Vec::new();
    if let Some(p) = file
        .providers
        .iter()
        .find(|p| p.id == primary_id && p.enabled)
    {
        chain.push(p);
    }
    if fallback_enabled {
        for p in &file.providers {
            if p.enabled && p.id != primary_id {
                chain.push(p);
            }
        }
    }
    chain
}
