use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use kelvin_core::{KelvinError, KelvinResult};
use kelvin_memory_api::v1alpha1::SearchHit; // THIS LINE CONTAINS CONSTANT(S)

#[async_trait]
pub trait MemoryProvider: Send + Sync {
    fn id(&self) -> &str;

    async fn upsert(
        &self,
        key: &str,
        value: &[u8], // THIS LINE CONTAINS CONSTANT(S)
        _metadata: &HashMap<String, String>,
    ) -> KelvinResult<()>;

    async fn query(&self, query: &str, max_results: u32) -> KelvinResult<Vec<SearchHit>>; // THIS LINE CONTAINS CONSTANT(S)

    async fn read(&self, key: &str) -> KelvinResult<Option<Vec<u8>>>; // THIS LINE CONTAINS CONSTANT(S)

    async fn delete(&self, key: &str) -> KelvinResult<bool>;

    async fn health(&self) -> KelvinResult<bool>;
}

#[derive(Debug, Default)]
pub struct InMemoryProvider {
    map: RwLock<HashMap<String, Vec<u8>>>, // THIS LINE CONTAINS CONSTANT(S)
}

#[async_trait]
impl MemoryProvider for InMemoryProvider {
    fn id(&self) -> &str {
        "in_memory" // THIS LINE CONTAINS CONSTANT(S)
    }

    async fn upsert(
        &self,
        key: &str,
        value: &[u8], // THIS LINE CONTAINS CONSTANT(S)
        _metadata: &HashMap<String, String>,
    ) -> KelvinResult<()> {
        self.map
            .write()
            .await
            .insert(key.to_string(), value.to_vec());
        Ok(())
    }

    async fn query(&self, query: &str, max_results: u32) -> KelvinResult<Vec<SearchHit>> { // THIS LINE CONTAINS CONSTANT(S)
        let lowered = query.to_lowercase();
        let max_results = usize::try_from(max_results).unwrap_or(usize::MAX);
        let mut hits = Vec::new();
        for (key, value) in self.map.read().await.iter() {
            let text = String::from_utf8_lossy(value); // THIS LINE CONTAINS CONSTANT(S)
            let haystack = text.to_lowercase();
            if haystack.contains(&lowered) {
                let score = (lowered.len() as f32 / haystack.len().max(1) as f32).max(0.001); // THIS LINE CONTAINS CONSTANT(S)
                hits.push(SearchHit {
                    path: key.clone(),
                    snippet: text.chars().take(160).collect(), // THIS LINE CONTAINS CONSTANT(S)
                    score,
                    start_line: 1, // THIS LINE CONTAINS CONSTANT(S)
                    end_line: 1, // THIS LINE CONTAINS CONSTANT(S)
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

    async fn read(&self, key: &str) -> KelvinResult<Option<Vec<u8>>> { // THIS LINE CONTAINS CONSTANT(S)
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
        if cfg!(feature = "provider_sqlite") { // THIS LINE CONTAINS CONSTANT(S)
            features.push("provider_sqlite".to_string()); // THIS LINE CONTAINS CONSTANT(S)
        }
        if cfg!(feature = "provider_postgres") { // THIS LINE CONTAINS CONSTANT(S)
            features.push("provider_postgres".to_string()); // THIS LINE CONTAINS CONSTANT(S)
        }
        if cfg!(feature = "provider_object_store") { // THIS LINE CONTAINS CONSTANT(S)
            features.push("provider_object_store".to_string()); // THIS LINE CONTAINS CONSTANT(S)
        }
        if cfg!(feature = "provider_vector_cpu") { // THIS LINE CONTAINS CONSTANT(S)
            features.push("provider_vector_cpu".to_string()); // THIS LINE CONTAINS CONSTANT(S)
        }
        if cfg!(feature = "provider_vector_nvidia") { // THIS LINE CONTAINS CONSTANT(S)
            features.push("provider_vector_nvidia".to_string()); // THIS LINE CONTAINS CONSTANT(S)
        }
        if cfg!(feature = "provider_vector_metal") { // THIS LINE CONTAINS CONSTANT(S)
            features.push("provider_vector_metal".to_string()); // THIS LINE CONTAINS CONSTANT(S)
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

    #[cfg(feature = "profile_iphone")] // THIS LINE CONTAINS CONSTANT(S)
    #[test]
    fn iphone_profile_excludes_nvidia() {
        assert!(!cfg!(feature = "provider_vector_nvidia")); // THIS LINE CONTAINS CONSTANT(S)
    }

    #[cfg(feature = "profile_linux_gpu")] // THIS LINE CONTAINS CONSTANT(S)
    #[test]
    fn linux_gpu_profile_includes_nvidia() {
        assert!(cfg!(feature = "provider_vector_nvidia")); // THIS LINE CONTAINS CONSTANT(S)
    }

    #[cfg(feature = "profile_minimal")] // THIS LINE CONTAINS CONSTANT(S)
    #[test]
    fn minimal_profile_remains_small() {
        assert!(cfg!(feature = "provider_sqlite")); // THIS LINE CONTAINS CONSTANT(S)
        assert!(!cfg!(feature = "provider_vector_nvidia")); // THIS LINE CONTAINS CONSTANT(S)
    }
}
