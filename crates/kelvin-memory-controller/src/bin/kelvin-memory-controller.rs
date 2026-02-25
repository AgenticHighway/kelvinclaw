use std::fs;
use std::net::SocketAddr;

use tonic::transport::Server;

use kelvin_memory_api::v1alpha1::memory_service_server::MemoryServiceServer;
use kelvin_memory_api::MemoryModuleManifest;
use kelvin_memory_controller::{MemoryController, MemoryControllerConfig, ProviderRegistry};

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let addr: SocketAddr = std::env::var("KELVIN_MEMORY_CONTROLLER_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:50051".to_string())
        .parse()?;

    let cfg = MemoryControllerConfig::from_env();
    if cfg.decoding_key_pem.trim().is_empty() {
        return Err("KELVIN_MEMORY_PUBLIC_KEY_PEM is required".into());
    }

    let controller = MemoryController::new(cfg, ProviderRegistry::with_default_in_memory())?;

    if let (Ok(manifest_path), Ok(wasm_path)) = (
        std::env::var("KELVIN_MEMORY_MODULE_MANIFEST"),
        std::env::var("KELVIN_MEMORY_MODULE_WASM"),
    ) {
        let manifest_bytes = fs::read(&manifest_path)?;
        let manifest: MemoryModuleManifest = serde_json::from_slice(&manifest_bytes)?;
        let wasm_bytes = fs::read(&wasm_path)?;
        controller
            .register_module_bytes(manifest, &wasm_bytes)
            .await?;
    }

    println!("kelvin-memory-controller listening on {addr}");
    Server::builder()
        .add_service(MemoryServiceServer::new(controller))
        .serve(addr)
        .await?;
    Ok(())
}
