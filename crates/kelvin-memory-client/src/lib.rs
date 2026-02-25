use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use jsonwebtoken::EncodingKey;
use tokio::sync::Mutex;
use tonic::transport::Channel;
use tonic::Request;

use kelvin_core::{
    KelvinError, KelvinResult, MemoryEmbeddingProbeResult, MemoryProviderStatus, MemoryReadParams,
    MemoryReadResult, MemorySearchManager, MemorySearchOptions, MemorySearchResult, MemorySource,
    MemorySyncParams,
};
use kelvin_memory_api::v1alpha1::memory_service_client::MemoryServiceClient;
use kelvin_memory_api::v1alpha1::{
    HealthRequest, QueryRequest, ReadRequest, RequestContext, SearchHit, UpsertRequest,
};
use kelvin_memory_api::{
    mint_delegation_token, new_request_id, DelegationClaims, MemoryOperation, RequestLimits,
};

#[derive(Debug, Clone)]
pub struct MemoryClientConfig {
    pub endpoint: String,
    pub issuer: String,
    pub audience: String,
    pub subject: String,
    pub tenant_id: String,
    pub workspace_id: String,
    pub session_id: String,
    pub module_id: String,
    pub signing_key_pem: String,
    pub timeout_ms: u64,
    pub max_bytes: u64,
    pub max_results: u32,
}

impl Default for MemoryClientConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://127.0.0.1:50051".to_string(),
            issuer: "kelvin-root".to_string(),
            audience: "kelvin-memory-controller".to_string(),
            subject: "kelvin-root-memory-client".to_string(),
            tenant_id: "default".to_string(),
            workspace_id: "default".to_string(),
            session_id: "default".to_string(),
            module_id: "memory.echo".to_string(),
            signing_key_pem: String::new(),
            timeout_ms: 2_000,
            max_bytes: 1024 * 1024,
            max_results: 20,
        }
    }
}

impl MemoryClientConfig {
    pub fn from_env() -> Self {
        let mut cfg = Self::default();
        if let Ok(value) = std::env::var("KELVIN_MEMORY_RPC_ENDPOINT") {
            if !value.trim().is_empty() {
                cfg.endpoint = value;
            }
        }
        if let Ok(value) = std::env::var("KELVIN_MEMORY_RPC_ISSUER") {
            if !value.trim().is_empty() {
                cfg.issuer = value;
            }
        }
        if let Ok(value) = std::env::var("KELVIN_MEMORY_RPC_AUDIENCE") {
            if !value.trim().is_empty() {
                cfg.audience = value;
            }
        }
        if let Ok(value) = std::env::var("KELVIN_MEMORY_RPC_SUBJECT") {
            if !value.trim().is_empty() {
                cfg.subject = value;
            }
        }
        if let Ok(value) = std::env::var("KELVIN_MEMORY_TENANT_ID") {
            if !value.trim().is_empty() {
                cfg.tenant_id = value;
            }
        }
        if let Ok(value) = std::env::var("KELVIN_MEMORY_WORKSPACE_ID") {
            if !value.trim().is_empty() {
                cfg.workspace_id = value;
            }
        }
        if let Ok(value) = std::env::var("KELVIN_MEMORY_SESSION_ID") {
            if !value.trim().is_empty() {
                cfg.session_id = value;
            }
        }
        if let Ok(value) = std::env::var("KELVIN_MEMORY_MODULE_ID") {
            if !value.trim().is_empty() {
                cfg.module_id = value;
            }
        }
        if let Ok(value) = std::env::var("KELVIN_MEMORY_SIGNING_KEY_PEM") {
            if !value.trim().is_empty() {
                cfg.signing_key_pem = value;
            }
        }
        if let Ok(value) = std::env::var("KELVIN_MEMORY_TIMEOUT_MS") {
            if let Ok(parsed) = value.parse::<u64>() {
                cfg.timeout_ms = parsed;
            }
        }
        if let Ok(value) = std::env::var("KELVIN_MEMORY_MAX_BYTES") {
            if let Ok(parsed) = value.parse::<u64>() {
                cfg.max_bytes = parsed;
            }
        }
        if let Ok(value) = std::env::var("KELVIN_MEMORY_MAX_RESULTS") {
            if let Ok(parsed) = value.parse::<u32>() {
                cfg.max_results = parsed;
            }
        }
        cfg
    }
}

pub struct RpcMemoryManager {
    cfg: MemoryClientConfig,
    signer: EncodingKey,
    client: Mutex<MemoryServiceClient<Channel>>,
}

impl RpcMemoryManager {
    pub async fn connect(cfg: MemoryClientConfig) -> KelvinResult<Self> {
        if cfg.signing_key_pem.trim().is_empty() {
            return Err(KelvinError::InvalidInput(
                "memory rpc signing key pem must be provided".to_string(),
            ));
        }
        let signer = EncodingKey::from_ed_pem(cfg.signing_key_pem.as_bytes()).map_err(|err| {
            KelvinError::InvalidInput(format!("invalid memory rpc signing key pem: {err}"))
        })?;
        let endpoint = cfg.endpoint.clone();
        let client = MemoryServiceClient::connect(endpoint.clone())
            .await
            .map_err(|err| {
                KelvinError::Backend(format!(
                    "memory controller unavailable at {endpoint}: {err}"
                ))
            })?;
        Ok(Self {
            cfg,
            signer,
            client: Mutex::new(client),
        })
    }

    fn build_context(
        &self,
        op: MemoryOperation,
        request_id: String,
    ) -> KelvinResult<RequestContext> {
        let now = now_secs();
        let allowed_capabilities = match op {
            MemoryOperation::Upsert | MemoryOperation::Delete => vec!["memory_crud".to_string()],
            MemoryOperation::Query | MemoryOperation::Read => vec!["memory_read".to_string()],
            MemoryOperation::Health => vec!["memory_health".to_string()],
        };
        let claims = DelegationClaims {
            iss: self.cfg.issuer.clone(),
            sub: self.cfg.subject.clone(),
            aud: self.cfg.audience.clone(),
            jti: format!("{}-{request_id}", op.as_str()),
            exp: now.saturating_add(60),
            nbf: now.saturating_sub(1),
            tenant_id: self.cfg.tenant_id.clone(),
            workspace_id: self.cfg.workspace_id.clone(),
            session_id: self.cfg.session_id.clone(),
            module_id: self.cfg.module_id.clone(),
            allowed_ops: vec![op.as_str().to_string()],
            allowed_capabilities,
            request_limits: RequestLimits {
                timeout_ms: self.cfg.timeout_ms,
                max_bytes: self.cfg.max_bytes,
                max_results: self.cfg.max_results,
            },
        };
        let token = mint_delegation_token(&claims, &self.signer)
            .map_err(|err| KelvinError::InvalidInput(format!("failed to mint token: {err}")))?;
        Ok(RequestContext {
            delegation_token: token,
            request_id,
            tenant_id: self.cfg.tenant_id.clone(),
            workspace_id: self.cfg.workspace_id.clone(),
            session_id: self.cfg.session_id.clone(),
            module_id: self.cfg.module_id.clone(),
        })
    }

    pub async fn upsert(&self, key: &str, value: &[u8]) -> KelvinResult<()> {
        let request_id = new_request_id();
        let context = self.build_context(MemoryOperation::Upsert, request_id)?;
        self.client
            .lock()
            .await
            .upsert(Request::new(UpsertRequest {
                context: Some(context),
                key: key.to_string(),
                value: value.to_vec(),
                metadata: Default::default(),
            }))
            .await
            .map_err(map_status)?;
        Ok(())
    }
}

#[async_trait]
impl MemorySearchManager for RpcMemoryManager {
    async fn search(
        &self,
        query: &str,
        opts: MemorySearchOptions,
    ) -> KelvinResult<Vec<MemorySearchResult>> {
        let request_id = new_request_id();
        let context = self.build_context(MemoryOperation::Query, request_id)?;
        let max_results = u32::try_from(opts.max_results)
            .unwrap_or(self.cfg.max_results)
            .min(self.cfg.max_results);
        let response = self
            .client
            .lock()
            .await
            .query(Request::new(QueryRequest {
                context: Some(context),
                query: query.to_string(),
                max_results,
            }))
            .await
            .map_err(map_status)?
            .into_inner();
        Ok(response
            .hits
            .into_iter()
            .map(map_search_hit)
            .collect::<Vec<_>>())
    }

    async fn read_file(&self, params: MemoryReadParams) -> KelvinResult<MemoryReadResult> {
        let request_id = new_request_id();
        let context = self.build_context(MemoryOperation::Read, request_id)?;
        let response = self
            .client
            .lock()
            .await
            .read(Request::new(ReadRequest {
                context: Some(context),
                key: params.rel_path.clone(),
            }))
            .await
            .map_err(map_status)?
            .into_inner();

        let text = if response.found {
            String::from_utf8(response.value).map_err(|err| {
                KelvinError::Backend(format!(
                    "memory controller returned non-utf8 payload: {err}"
                ))
            })?
        } else {
            String::new()
        };
        Ok(MemoryReadResult {
            text,
            path: params.rel_path,
        })
    }

    fn status(&self) -> MemoryProviderStatus {
        MemoryProviderStatus {
            backend: "rpc".to_string(),
            provider: "kelvin-memory-controller".to_string(),
            model: None,
            requested_provider: Some("memory-controller".to_string()),
            files: None,
            chunks: None,
            dirty: false,
            fallback: None,
            custom: serde_json::json!({
                "endpoint": self.cfg.endpoint,
                "module_id": self.cfg.module_id,
            }),
        }
    }

    async fn sync(&self, _params: Option<MemorySyncParams>) -> KelvinResult<()> {
        let request_id = new_request_id();
        let context = self.build_context(MemoryOperation::Health, request_id)?;
        self.client
            .lock()
            .await
            .health(Request::new(HealthRequest {
                context: Some(context),
            }))
            .await
            .map_err(map_status)?;
        Ok(())
    }

    async fn probe_embedding_availability(&self) -> KelvinResult<MemoryEmbeddingProbeResult> {
        Ok(MemoryEmbeddingProbeResult {
            ok: false,
            error: Some("embedding is provider-specific and not enabled in rpc mvp".to_string()),
        })
    }

    async fn probe_vector_availability(&self) -> KelvinResult<bool> {
        Ok(false)
    }
}

fn map_search_hit(hit: SearchHit) -> MemorySearchResult {
    MemorySearchResult {
        path: hit.path,
        start_line: hit.start_line as usize,
        end_line: hit.end_line as usize,
        score: hit.score,
        snippet: hit.snippet,
        source: MemorySource::Memory,
        citation: None,
    }
}

fn map_status(status: tonic::Status) -> KelvinError {
    if status.code() == tonic::Code::DeadlineExceeded {
        KelvinError::Timeout(status.message().to_string())
    } else if status.code() == tonic::Code::InvalidArgument {
        KelvinError::InvalidInput(status.message().to_string())
    } else if status.code() == tonic::Code::NotFound {
        KelvinError::NotFound(status.message().to_string())
    } else {
        KelvinError::Backend(format!(
            "memory controller unavailable: {}",
            status.message()
        ))
    }
}

fn now_secs() -> usize {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as usize)
        .unwrap_or_default()
}
