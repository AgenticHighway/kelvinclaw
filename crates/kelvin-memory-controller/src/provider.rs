use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use kelvin_core::{KelvinError, KelvinResult};
use kelvin_memory_api::v1alpha1::SearchHit;

use crate::consts;

#[async_trait]
pub trait MemoryProvider: Send + Sync {
    fn id(&self) -> &str;

    async fn upsert(
        &self,
        key: &str,
        value: &[u8],
        _metadata: &HashMap<String, String>,
    ) -> KelvinResult<()>;

    async fn query(&self, query: &str, max_results: u32) -> KelvinResult<Vec<SearchHit>>;

    async fn read(&self, key: &str) -> KelvinResult<Option<Vec<u8>>>;

    async fn delete(&self, key: &str) -> KelvinResult<bool>;

    async fn health(&self) -> KelvinResult<bool>;
}

#[derive(Debug, Default)]
pub struct InMemoryProvider {
    map: RwLock<HashMap<String, Vec<u8>>>,
}

#[async_trait]
impl MemoryProvider for InMemoryProvider {
    fn id(&self) -> &str {
        consts::IN_MEMORY_PROVIDER_ID
    }

    async fn upsert(
        &self,
        key: &str,
        value: &[u8],
        _metadata: &HashMap<String, String>,
    ) -> KelvinResult<()> {
        self.map
            .write()
            .await
            .insert(key.to_string(), value.to_vec());
        Ok(())
    }

    async fn query(&self, query: &str, max_results: u32) -> KelvinResult<Vec<SearchHit>> {
        let lowered = query.to_lowercase();
        let max_results = usize::try_from(max_results).unwrap_or(usize::MAX);
        let mut hits = Vec::new();
        for (key, value) in self.map.read().await.iter() {
            let text = String::from_utf8_lossy(value);
            let haystack = text.to_lowercase();
            if haystack.contains(&lowered) {
                let score = (lowered.len() as f32 / haystack.len().max(1) as f32)
                    .max(consts::MIN_SEARCH_SCORE);
                hits.push(SearchHit {
                    path: key.clone(),
                    snippet: text
                        .chars()
                        .take(consts::SEARCH_SNIPPET_CHAR_LIMIT)
                        .collect(),
                    score,
                    start_line: consts::DEFAULT_SEARCH_START_LINE,
                    end_line: consts::DEFAULT_SEARCH_END_LINE,
                });
            }
        }
        hits.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.path.cmp(&b.path))
                .then_with(|| a.snippet.cmp(&b.snippet))
        });
        hits.truncate(max_results);
        Ok(hits)
    }

    async fn read(&self, key: &str) -> KelvinResult<Option<Vec<u8>>> {
        Ok(self.map.read().await.get(key).cloned())
    }

    async fn delete(&self, key: &str) -> KelvinResult<bool> {
        Ok(self.map.write().await.remove(key).is_some())
    }

    async fn health(&self) -> KelvinResult<bool> {
        Ok(true)
    }
}

#[derive(Default)]
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn MemoryProvider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_default_in_memory() -> Self {
        let mut this = Self::new();
        this.register(Arc::new(InMemoryProvider::default()));
        this
    }

    pub fn register(&mut self, provider: Arc<dyn MemoryProvider>) {
        self.providers.insert(provider.id().to_string(), provider);
    }

    pub fn get(&self, id: &str) -> KelvinResult<Arc<dyn MemoryProvider>> {
        self.providers
            .get(id)
            .cloned()
            .ok_or_else(|| KelvinError::NotFound(format!("memory provider not found: {id}")))
    }

    pub fn primary(&self) -> KelvinResult<Arc<dyn MemoryProvider>> {
        self.providers
            .values()
            .next()
            .cloned()
            .ok_or_else(|| KelvinError::InvalidInput("no memory providers registered".to_string()))
    }

    pub fn available_features(&self) -> Vec<String> {
        let mut features = Vec::new();
        if cfg!(feature = "provider_sqlite") {
            features.push("provider_sqlite".to_string());
        }
        if cfg!(feature = "provider_postgres") {
            features.push("provider_postgres".to_string());
        }
        if cfg!(feature = "provider_object_store") {
            features.push("provider_object_store".to_string());
        }
        if cfg!(feature = "provider_vector_cpu") {
            features.push("provider_vector_cpu".to_string());
        }
        if cfg!(feature = "provider_vector_nvidia") {
            features.push("provider_vector_nvidia".to_string());
        }
        if cfg!(feature = "provider_vector_metal") {
            features.push("provider_vector_metal".to_string());
        }
        features.sort();
        features.dedup();
        features
    }
}

#[cfg(test)]
mod tests {
    use super::ProviderRegistry;

    #[test]
    fn available_features_are_sorted_and_unique() {
        let features = ProviderRegistry::with_default_in_memory().available_features();
        let mut sorted = features.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(features, sorted);
    }

    #[cfg(feature = "profile_iphone")]
    #[test]
    fn iphone_profile_excludes_nvidia() {
        assert!(!cfg!(feature = "provider_vector_nvidia"));
    }

    #[cfg(feature = "profile_linux_gpu")]
    #[test]
    fn linux_gpu_profile_includes_nvidia() {
        assert!(cfg!(feature = "provider_vector_nvidia"));
    }

    #[cfg(feature = "profile_minimal")]
    #[test]
    fn minimal_profile_remains_small() {
        assert!(cfg!(feature = "provider_sqlite"));
        assert!(!cfg!(feature = "provider_vector_nvidia"));
    }
}
