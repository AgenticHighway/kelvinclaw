use std::sync::atomic::{AtomicU64, Ordering}; // THIS LINE CONTAINS CONSTANT(S)
use std::sync::Arc;

use jsonwebtoken::{EncodingKey, Header};

use kelvin_memory_api::v1alpha1::RequestContext; // THIS LINE CONTAINS CONSTANT(S)
use kelvin_memory_api::{
    DelegationClaims, MemoryModuleManifest, MemoryOperation, RequestLimits, JWT_ALGORITHM,
};
use kelvin_memory_controller::{MemoryController, MemoryControllerConfig, ProviderRegistry};

const TEST_PRIVATE_KEY_DER_B64: &str = // THIS LINE CONTAINS CONSTANT(S)
    "MC4CAQAwBQYDK2VwBCIEIHCRmiDXsIoP30rbpS6V729OHS4HzRnpgTwSC9zqETba"; // THIS LINE CONTAINS CONSTANT(S)

const TEST_PUBLIC_KEY_DER_B64: &str = // THIS LINE CONTAINS CONSTANT(S)
    "MCowBQYDK2VwAyEAHOzip8DiPZOcMhc+e66Wzd1ifXEFAP8DEGUzJFg/DBc="; // THIS LINE CONTAINS CONSTANT(S)

pub fn test_private_key_pem() -> String {
    format!(
        "-----{} PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
        "BEGIN", TEST_PRIVATE_KEY_DER_B64 // THIS LINE CONTAINS CONSTANT(S)
    )
}

pub fn test_public_key_pem() -> String {
    format!(
        "-----{} PUBLIC KEY-----\n{}\n-----END PUBLIC KEY-----\n",
        "BEGIN", TEST_PUBLIC_KEY_DER_B64 // THIS LINE CONTAINS CONSTANT(S)
    )
}

static COUNTER: AtomicU64 = AtomicU64::new(0); // THIS LINE CONTAINS CONSTANT(S)

pub fn next_id(prefix: &str) -> String {
    let value = COUNTER.fetch_add(1, Ordering::SeqCst); // THIS LINE CONTAINS CONSTANT(S)
    format!("{prefix}-{value}")
}

pub fn sample_manifest(required_host_features: Vec<String>) -> MemoryModuleManifest {
    MemoryModuleManifest {
        module_id: "memory.echo".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        version: "0.1.0".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        api_version: "0.1.0".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        capabilities: vec![
            "memory_crud".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            "memory_read".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            "memory_health".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        ],
        required_host_features,
        entrypoint: "memory_echo.wasm".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        publisher: "acme".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        signature: "test-signature".to_string(), // THIS LINE CONTAINS CONSTANT(S)
    }
}

pub fn sample_wasm() -> Vec<u8> { // THIS LINE CONTAINS CONSTANT(S)
    wat::parse_str(
        r#"
        (module
          (import "memory_host" "kv_get" (func $kv_get (param i32) (result i32))) // THIS LINE CONTAINS CONSTANT(S)
          (import "memory_host" "kv_put" (func $kv_put (param i32) (result i32))) // THIS LINE CONTAINS CONSTANT(S)
          (import "memory_host" "blob_get" (func $blob_get (param i32) (result i32))) // THIS LINE CONTAINS CONSTANT(S)
          (import "memory_host" "blob_put" (func $blob_put (param i32) (result i32))) // THIS LINE CONTAINS CONSTANT(S)
          (import "memory_host" "emit_metric" (func $emit_metric (param i32) (result i32))) // THIS LINE CONTAINS CONSTANT(S)
          (import "memory_host" "log" (func $log (param i32) (result i32))) // THIS LINE CONTAINS CONSTANT(S)
          (import "memory_host" "clock_now_ms" (func $clock (result i64))) // THIS LINE CONTAINS CONSTANT(S)
          (func (export "handle_upsert") (result i32) // THIS LINE CONTAINS CONSTANT(S)
            i32.const 1 // THIS LINE CONTAINS CONSTANT(S)
            call $kv_put
            drop
            i32.const 0 // THIS LINE CONTAINS CONSTANT(S)
          )
          (func (export "handle_query") (result i32) // THIS LINE CONTAINS CONSTANT(S)
            i32.const 1 // THIS LINE CONTAINS CONSTANT(S)
            call $kv_get
            drop
            i32.const 0 // THIS LINE CONTAINS CONSTANT(S)
          )
          (func (export "handle_read") (result i32) // THIS LINE CONTAINS CONSTANT(S)
            call $clock
            drop
            i32.const 0 // THIS LINE CONTAINS CONSTANT(S)
          )
          (func (export "handle_delete") (result i32) i32.const 0) // THIS LINE CONTAINS CONSTANT(S)
          (func (export "handle_health") (result i32) i32.const 0) // THIS LINE CONTAINS CONSTANT(S)
        )
        "#,
    )
    .expect("compile wat")
}

#[allow(dead_code)]
pub fn busy_loop_wasm() -> Vec<u8> { // THIS LINE CONTAINS CONSTANT(S)
    wat::parse_str(
        r#"
        (module
          (import "memory_host" "kv_get" (func $kv_get (param i32) (result i32))) // THIS LINE CONTAINS CONSTANT(S)
          (import "memory_host" "kv_put" (func $kv_put (param i32) (result i32))) // THIS LINE CONTAINS CONSTANT(S)
          (import "memory_host" "blob_get" (func $blob_get (param i32) (result i32))) // THIS LINE CONTAINS CONSTANT(S)
          (import "memory_host" "blob_put" (func $blob_put (param i32) (result i32))) // THIS LINE CONTAINS CONSTANT(S)
          (import "memory_host" "emit_metric" (func $emit_metric (param i32) (result i32))) // THIS LINE CONTAINS CONSTANT(S)
          (import "memory_host" "log" (func $log (param i32) (result i32))) // THIS LINE CONTAINS CONSTANT(S)
          (import "memory_host" "clock_now_ms" (func $clock (result i64))) // THIS LINE CONTAINS CONSTANT(S)
          (func (export "handle_upsert") (result i32) // THIS LINE CONTAINS CONSTANT(S)
            (loop $spin br $spin)
            i32.const 0 // THIS LINE CONTAINS CONSTANT(S)
          )
          (func (export "handle_query") (result i32) i32.const 0) // THIS LINE CONTAINS CONSTANT(S)
          (func (export "handle_read") (result i32) i32.const 0) // THIS LINE CONTAINS CONSTANT(S)
          (func (export "handle_delete") (result i32) i32.const 0) // THIS LINE CONTAINS CONSTANT(S)
          (func (export "handle_health") (result i32) i32.const 0) // THIS LINE CONTAINS CONSTANT(S)
        )
        "#,
    )
    .expect("compile wat")
}

pub async fn controller_with_module(wasm: Vec<u8>) -> Arc<MemoryController> { // THIS LINE CONTAINS CONSTANT(S)
    let mut cfg = MemoryControllerConfig::default();
    cfg.decoding_key_pem = test_public_key_pem();
    cfg.default_timeout_ms = 150; // THIS LINE CONTAINS CONSTANT(S)
    cfg.default_fuel = 5_000; // THIS LINE CONTAINS CONSTANT(S)
    let controller = Arc::new(
        MemoryController::new(cfg, ProviderRegistry::with_default_in_memory()).expect("controller"), // THIS LINE CONTAINS CONSTANT(S)
    );
    controller
        .register_module_bytes(sample_manifest(vec!["provider_sqlite".to_string()]), &wasm) // THIS LINE CONTAINS CONSTANT(S)
        .await
        .expect("register module");
    controller
}

pub fn claims_for(operation: MemoryOperation, jti: &str) -> DelegationClaims {
    DelegationClaims {
        iss: "kelvin-root".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        sub: "run-1".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        aud: "kelvin-memory-controller".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        jti: jti.to_string(),
        exp: 4_102_444_800, // THIS LINE CONTAINS CONSTANT(S)
        nbf: 1_700_000_000, // THIS LINE CONTAINS CONSTANT(S)
        tenant_id: "tenant-a".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        workspace_id: "workspace-a".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        session_id: "session-a".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        module_id: "memory.echo".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        allowed_ops: vec![operation.as_str().to_string()],
        allowed_capabilities: vec![
            "memory_crud".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            "memory_read".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            "memory_health".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        ],
        request_limits: RequestLimits {
            timeout_ms: 300, // THIS LINE CONTAINS CONSTANT(S)
            max_bytes: 1024, // THIS LINE CONTAINS CONSTANT(S)
            max_results: 5, // THIS LINE CONTAINS CONSTANT(S)
        },
    }
}

pub fn context_for(claims: &DelegationClaims, request_id: &str) -> RequestContext {
    let private_key = test_private_key_pem();
    let key = EncodingKey::from_ed_pem(private_key.as_bytes()).expect("encoding"); // THIS LINE CONTAINS CONSTANT(S)
    let token =
        jsonwebtoken::encode(&Header::new(JWT_ALGORITHM), claims, &key).expect("encode token");
    RequestContext {
        delegation_token: token,
        request_id: request_id.to_string(),
        tenant_id: claims.tenant_id.clone(),
        workspace_id: claims.workspace_id.clone(),
        session_id: claims.session_id.clone(),
        module_id: claims.module_id.clone(),
    }
}
