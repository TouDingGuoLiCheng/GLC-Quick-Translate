use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct CacheEntry {
    pub translated: String,
    pub provider: String,
    pub created_at: Instant,
}

pub struct TranslateCache {
    inner: Mutex<HashMap<String, CacheEntry>>,
}

impl TranslateCache {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    pub fn get(&self, key: &str, ttl: Duration) -> Option<CacheEntry> {
        let guard = self.inner.lock().ok()?;
        let entry = guard.get(key)?;
        if entry.created_at.elapsed() > ttl {
            return None;
        }
        Some(entry.clone())
    }

    pub fn set(&self, key: String, translated: String, provider: String) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.insert(
                key,
                CacheEntry {
                    translated,
                    provider,
                    created_at: Instant::now(),
                },
            );
        }
    }
}

pub fn cache_key(text: &str, target_lang: &str) -> String {
    format!("{target_lang}:{text}")
}
