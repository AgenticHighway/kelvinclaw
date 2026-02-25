use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;

use kelvin_core::{MemoryReadParams, MemorySearchManager};
use kelvin_memory_api::v1alpha1::memory_service_server::MemoryServiceServer;
use kelvin_memory_api::MemoryModuleManifest;
use kelvin_memory_client::{MemoryClientConfig, RpcMemoryManager};
use kelvin_memory_controller::{MemoryController, MemoryControllerConfig, ProviderRegistry};

const TEST_PRIVATE_KEY_PEM: &str = r#"-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VwBCIEIHCRmiDXsIoP30rbpS6V729OHS4HzRnpgTwSC9zqETba
-----END PRIVATE KEY-----
"#;

const TEST_PUBLIC_KEY_PEM: &str = r#"-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEAHOzip8DiPZOcMhc+e66Wzd1ifXEFAP8DEGUzJFg/DBc=
-----END PUBLIC KEY-----
"#;

fn sample_manifest() -> MemoryModuleManifest {
    MemoryModuleManifest {
        module_id: "memory.echo".to_string(),
        version: "0.1.0".to_string(),
        api_version: "0.1.0".to_string(),
        capabilities: vec![
            "memory_crud".to_string(),
            "memory_read".to_string(),
            "memory_health".to_string(),
        ],
        required_host_features: vec!["provider_sqlite".to_string()],
        entrypoint: "memory_echo.wasm".to_string(),
        publisher: "acme".to_string(),
        signature: "test-signature".to_string(),
    }
}

fn sample_wasm() -> Vec<u8> {
    wat::parse_str(
        r#"
        (module
          (import "memory_host" "kv_get" (func $kv_get (param i32) (result i32)))
          (import "memory_host" "kv_put" (func $kv_put (param i32) (result i32)))
          (import "memory_host" "blob_get" (func $blob_get (param i32) (result i32)))
          (import "memory_host" "blob_put" (func $blob_put (param i32) (result i32)))
          (import "memory_host" "emit_metric" (func $emit_metric (param i32) (result i32)))
          (import "memory_host" "log" (func $log (param i32) (result i32)))
          (import "memory_host" "clock_now_ms" (func $clock (result i64)))
          (func (export "handle_upsert") (result i32) i32.const 0)
          (func (export "handle_query") (result i32) i32.const 0)
          (func (export "handle_read") (result i32) i32.const 0)
          (func (export "handle_delete") (result i32) i32.const 0)
          (func (export "handle_health") (result i32) i32.const 0)
        )
        "#,
    )
    .expect("compile wat")
}

async fn start_test_server() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("local addr");

    let mut cfg = MemoryControllerConfig::default();
    cfg.decoding_key_pem = TEST_PUBLIC_KEY_PEM.to_string();
    let controller =
        MemoryController::new(cfg, ProviderRegistry::with_default_in_memory()).expect("controller");
    controller
        .register_module_bytes(sample_manifest(), &sample_wasm())
        .await
        .expect("register module");

    tokio::spawn(async move {
        Server::builder()
            .add_service(MemoryServiceServer::new(controller))
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await
            .expect("serve");
    });

    addr
}

#[tokio::test]
async fn rpc_memory_manager_crud_and_search_roundtrip() {
    let addr = start_test_server().await;
    let cfg = MemoryClientConfig {
        endpoint: format!("http://{addr}"),
        signing_key_pem: TEST_PRIVATE_KEY_PEM.to_string(),
        tenant_id: "tenant-a".to_string(),
        workspace_id: "workspace-a".to_string(),
        session_id: "session-a".to_string(),
        module_id: "memory.echo".to_string(),
        ..Default::default()
    };
    let manager = RpcMemoryManager::connect(cfg).await.expect("connect");

    manager
        .upsert("MEMORY.md", b"configured router on vlan10")
        .await
        .expect("upsert");

    let hits = manager
        .search("router", Default::default())
        .await
        .expect("search");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].path, "MEMORY.md");

    let read = manager
        .read_file(MemoryReadParams {
            rel_path: "MEMORY.md".to_string(),
            from: None,
            lines: None,
        })
        .await
        .expect("read");
    assert!(read.text.contains("vlan10"));
}

#[tokio::test]
async fn rpc_memory_manager_unavailable_returns_typed_backend_error() {
    let cfg = MemoryClientConfig {
        endpoint: "http://127.0.0.1:65534".to_string(),
        signing_key_pem: TEST_PRIVATE_KEY_PEM.to_string(),
        ..Default::default()
    };
    let err = match RpcMemoryManager::connect(cfg).await {
        Ok(_) => panic!("connect should fail"),
        Err(err) => err,
    };
    assert!(err
        .to_string()
        .to_lowercase()
        .contains("memory controller unavailable"));
}
