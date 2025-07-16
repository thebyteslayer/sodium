// Copyright (c) 2025, TheByteSlayer, Sodium
// A scalable and optimized Key Value Caching System, written in Rust.

use crate::core::{CacheError, get_cache};

#[derive(Debug, Clone)]
pub enum SearchType {
    Key,
    Value,
    KeyOrValue,
    KeyAndValue,
}

impl SearchType {
    pub fn parse(input: &str) -> Result<Self, String> {
        match input.trim().to_lowercase().as_str() {
            "key" => Ok(SearchType::Key),
            "value" => Ok(SearchType::Value),
            "key or value" => Ok(SearchType::KeyOrValue),
            "key and value" => Ok(SearchType::KeyAndValue),
            _ => Err(format!("Invalid search type: {}. Valid types are: key, value, key or value, key and value", input)),
        }
    }
}

pub struct SearchEngine;

impl SearchEngine {
    pub async fn search_multiple(search_type: SearchType, queries: &[String]) -> Result<Vec<String>, CacheError> {
        let cache = get_cache();
        let queries_lower: Vec<String> = queries.iter().map(|q| q.to_lowercase()).collect();
        
        // Get all key-value pairs from cache
        let all_keys = cache.keys().await?;
        let mut matching_keys = Vec::new();
        
        for key in all_keys {
            let should_include = match &search_type {
                SearchType::Key => {
                    Self::key_contains_all(&key, &queries_lower)
                }
                SearchType::Value => {
                    if let Ok(value) = cache.get(&key).await {
                        Self::value_contains_all(&value, &queries_lower)
                    } else {
                        false
                    }
                }
                SearchType::KeyOrValue => {
                    let key_matches = Self::key_contains_all(&key, &queries_lower);
                    let value_matches = if let Ok(value) = cache.get(&key).await {
                        Self::value_contains_all(&value, &queries_lower)
                    } else {
                        false
                    };
                    key_matches || value_matches
                }
                SearchType::KeyAndValue => {
                    let key_matches = Self::key_contains_all(&key, &queries_lower);
                    let value_matches = if let Ok(value) = cache.get(&key).await {
                        Self::value_contains_all(&value, &queries_lower)
                    } else {
                        false
                    };
                    key_matches && value_matches
                }
            };
            
            if should_include {
                matching_keys.push(key);
            }
        }
        
        Ok(matching_keys)
    }


    
    fn key_contains_all(key: &str, queries: &[String]) -> bool {
        let key_lower = key.to_lowercase();
        queries.iter().all(|query| key_lower.contains(query))
    }
    
    fn value_contains_all(value: &str, queries: &[String]) -> bool {
        let value_lower = value.to_lowercase();
        queries.iter().all(|query| value_lower.contains(query))
    }
    

}



pub fn execute_search_multiple(search_type: SearchType, queries: Vec<String>) -> super::threading::TaskResult<Vec<String>> {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            handle.block_on(async {
                SearchEngine::search_multiple(search_type, &queries).await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            })
        }
        Err(_) => {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                SearchEngine::search_multiple(search_type, &queries).await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            })
        }
    }
} 