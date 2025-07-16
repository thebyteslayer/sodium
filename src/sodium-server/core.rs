// Copyright (c) 2025, TheByteSlayer, Sodium
// A scalable and optimized Key Value Caching System, written in Rust.

use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::atomic::{AtomicU64, Ordering};
use dashmap::DashMap;

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("Key not found: {0}")]
    KeyNotFound(String),
}

#[derive(Debug)]
struct CacheEntry {
    value: String,
    accessed_at: AtomicU64,
}

impl CacheEntry {
    fn new(value: String) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Self {
            value,
            accessed_at: AtomicU64::new(now),
        }
    }

    fn update_access_time(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.accessed_at.store(now, Ordering::Relaxed);
    }
}

#[derive(Debug)]
pub struct Sodium {
    storage: DashMap<String, CacheEntry>,
    total_operations: AtomicU64,
    hit_count: AtomicU64,
    miss_count: AtomicU64,
}

impl Sodium {
    pub fn new() -> Self {
        Self {
            storage: DashMap::new(),
            total_operations: AtomicU64::new(0),
            hit_count: AtomicU64::new(0),
            miss_count: AtomicU64::new(0),
        }
    }

    pub async fn set(&self, key: String, value: String) -> Result<(), CacheError> {
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        
        let entry = CacheEntry::new(value);
        self.storage.insert(key, entry);
        
        Ok(())
    }

    pub async fn get(&self, key: &str) -> Result<String, CacheError> {
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        
        if let Some(entry) = self.storage.get(key) {
            entry.update_access_time();
            self.hit_count.fetch_add(1, Ordering::Relaxed);
            Ok(entry.value.clone())
        } else {
            self.miss_count.fetch_add(1, Ordering::Relaxed);
            Err(CacheError::KeyNotFound(key.to_string()))
        }
    }

    pub async fn delete(&self, key: &str) -> Result<bool, CacheError> {
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        
        match self.storage.remove(key) {
            Some(_) => Ok(true),
            None => Ok(false),
        }
    }

    pub async fn keys(&self) -> Result<Vec<String>, CacheError> {
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        
        let keys: Vec<String> = self.storage.iter()
            .map(|entry| entry.key().clone())
            .collect();
        
        Ok(keys)
    }
}

impl Default for Sodium {
    fn default() -> Self {
        Self::new()
    }
}

static GLOBAL_CACHE: OnceLock<Arc<Sodium>> = OnceLock::new();

pub fn initialize_cache() {
    let _ = GLOBAL_CACHE.set(Arc::new(Sodium::new()));
}

pub fn get_cache() -> &'static Arc<Sodium> {
    GLOBAL_CACHE.get().expect("Cache not initialized")
}

pub fn execute_get(key: &str) -> super::threading::TaskResult<Option<String>> {
    let cache = get_cache();
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            handle.block_on(async {
                match cache.get(key).await {
                    Ok(value) => Ok(Some(value)),
                    Err(CacheError::KeyNotFound(_)) => Ok(None),
                }
            })
        }
        Err(_) => {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                match cache.get(key).await {
                    Ok(value) => Ok(Some(value)),
                    Err(CacheError::KeyNotFound(_)) => Ok(None),
                }
            })
        }
    }
}

pub fn execute_set(key: String, value: String) -> super::threading::TaskResult<()> {
    let cache = get_cache();
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            handle.block_on(async {
                cache.set(key, value).await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            })
        }
        Err(_) => {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                cache.set(key, value).await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            })
        }
    }
}

pub fn execute_delete(key: &str) -> super::threading::TaskResult<bool> {
    let cache = get_cache();
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            handle.block_on(async {
                cache.delete(key).await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            })
        }
        Err(_) => {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                cache.delete(key).await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            })
        }
    }
}

pub fn execute_keys() -> super::threading::TaskResult<Vec<String>> {
    let cache = get_cache();
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            handle.block_on(async {
                cache.keys().await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            })
        }
        Err(_) => {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                cache.keys().await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            })
        }
    }
}

