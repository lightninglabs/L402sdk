//! In-memory LRU token cache.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use l402_proto::ClientError;
use l402_proto::port::TokenStore;

/// In-memory token cache with a maximum capacity.
///
/// Thread-safe via `Arc<RwLock<>>`. Tokens are evicted in FIFO order
/// when the cache exceeds its capacity.
#[derive(Debug, Clone)]
pub struct InMemoryTokenStore {
    inner: Arc<RwLock<CacheInner>>,
}

#[derive(Debug)]
struct CacheInner {
    tokens: HashMap<String, (String, String)>,
    insertion_order: Vec<String>,
    capacity: usize,
}

impl InMemoryTokenStore {
    /// Create a new in-memory token store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(CacheInner {
                tokens: HashMap::with_capacity(capacity),
                insertion_order: Vec::with_capacity(capacity),
                capacity,
            })),
        }
    }
}

impl Default for InMemoryTokenStore {
    fn default() -> Self {
        Self::new(1024)
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl TokenStore for InMemoryTokenStore {
    async fn put(&self, endpoint: &str, macaroon: &str, preimage: &str) -> Result<(), ClientError> {
        let mut inner = self.inner.write().expect("RwLock poisoned");

        // Evict oldest entry if at capacity and this is a new key
        if inner.tokens.len() >= inner.capacity && !inner.tokens.contains_key(endpoint) {
            if let Some(oldest) = inner.insertion_order.first().cloned() {
                inner.tokens.remove(&oldest);
                inner.insertion_order.remove(0);
            }
        }

        // Remove from insertion order if updating existing key
        if inner.tokens.contains_key(endpoint) {
            inner.insertion_order.retain(|k| k != endpoint);
        }

        inner.tokens.insert(
            endpoint.to_string(),
            (macaroon.to_string(), preimage.to_string()),
        );
        inner.insertion_order.push(endpoint.to_string());

        Ok(())
    }

    async fn get(&self, endpoint: &str) -> Result<Option<(String, String)>, ClientError> {
        let inner = self.inner.read().expect("RwLock poisoned");
        Ok(inner.tokens.get(endpoint).cloned())
    }

    async fn remove(&self, endpoint: &str) -> Result<(), ClientError> {
        let mut inner = self.inner.write().expect("RwLock poisoned");
        inner.tokens.remove(endpoint);
        inner.insertion_order.retain(|k| k != endpoint);
        Ok(())
    }

    async fn clear(&self) -> Result<(), ClientError> {
        let mut inner = self.inner.write().expect("RwLock poisoned");
        inner.tokens.clear();
        inner.insertion_order.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn basic_put_get() {
        let store = InMemoryTokenStore::new(10);
        store
            .put("https://api.example.com/resource", "mac1", "pre1")
            .await
            .unwrap();

        let result = store.get("https://api.example.com/resource").await.unwrap();
        assert_eq!(result, Some(("mac1".to_string(), "pre1".to_string())));
    }

    #[tokio::test]
    async fn cache_miss() {
        let store = InMemoryTokenStore::new(10);
        let result = store.get("https://api.example.com/missing").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn eviction_at_capacity() {
        let store = InMemoryTokenStore::new(2);
        store.put("a", "mac_a", "pre_a").await.unwrap();
        store.put("b", "mac_b", "pre_b").await.unwrap();
        store.put("c", "mac_c", "pre_c").await.unwrap();

        // "a" should have been evicted
        assert!(store.get("a").await.unwrap().is_none());
        assert!(store.get("b").await.unwrap().is_some());
        assert!(store.get("c").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn remove_and_clear() {
        let store = InMemoryTokenStore::new(10);
        store.put("a", "mac", "pre").await.unwrap();

        store.remove("a").await.unwrap();
        assert!(store.get("a").await.unwrap().is_none());

        store.put("b", "mac", "pre").await.unwrap();
        store.clear().await.unwrap();
        assert!(store.get("b").await.unwrap().is_none());
    }
}
