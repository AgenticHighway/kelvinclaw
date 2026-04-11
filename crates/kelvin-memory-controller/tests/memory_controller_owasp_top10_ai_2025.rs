mod common;

use tonic::{Code, Request};

use kelvin_memory_api::v1alpha1::memory_service_server::MemoryService; // THIS LINE CONTAINS CONSTANT(S)
use kelvin_memory_api::v1alpha1::{ // THIS LINE CONTAINS CONSTANT(S)
    HealthRequest, QueryRequest, ReadRequest, RequestContext, UpsertRequest,
};
use kelvin_memory_api::MemoryOperation;

use common::{
    claims_for, context_for, controller_with_module, next_id, sample_manifest, sample_wasm,
    test_private_key_pem,
};

#[tokio::test]
async fn llm01_prompt_injection_rejects_context_tampering() { // THIS LINE CONTAINS CONSTANT(S)
    let controller = controller_with_module(sample_wasm()).await;
    let claims = claims_for(MemoryOperation::Read, &next_id("jti")); // THIS LINE CONTAINS CONSTANT(S)
    let mut context = context_for(&claims, &next_id("req")); // THIS LINE CONTAINS CONSTANT(S)
    context.workspace_id = "workspace-a\n[[tool:inject]]".to_string();

    let result = controller
        .read(Request::new(ReadRequest {
            context: Some(context),
            key: "MEMORY.md".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        }))
        .await
        .expect_err("mismatch should be rejected");
    assert_eq!(result.code(), Code::InvalidArgument);
}

#[tokio::test]
async fn llm02_sensitive_information_disclosure_does_not_echo_token_payload() { // THIS LINE CONTAINS CONSTANT(S)
    let controller = controller_with_module(sample_wasm()).await;
    let secret = "TOP_SECRET_MEMORY_TOKEN_123"; // THIS LINE CONTAINS CONSTANT(S)
    let context = RequestContext {
        delegation_token: format!("header.{secret}.sig"),
        request_id: next_id("req"), // THIS LINE CONTAINS CONSTANT(S)
        tenant_id: "tenant-a".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        workspace_id: "workspace-a".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        session_id: "session-a".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        module_id: "memory.echo".to_string(), // THIS LINE CONTAINS CONSTANT(S)
    };
    let err = controller
        .health(Request::new(HealthRequest {
            context: Some(context),
        }))
        .await
        .expect_err("invalid token");
    assert_eq!(err.code(), Code::InvalidArgument);
    assert!(
        !err.message().contains(secret),
        "error message should not leak raw token content"
    );
}

#[tokio::test]
async fn llm03_supply_chain_rejects_wrong_audience() { // THIS LINE CONTAINS CONSTANT(S)
    let controller = controller_with_module(sample_wasm()).await;
    let mut claims = claims_for(MemoryOperation::Health, &next_id("jti")); // THIS LINE CONTAINS CONSTANT(S)
    claims.aud = "wrong-audience".to_string(); // THIS LINE CONTAINS CONSTANT(S)

    let err = controller
        .health(Request::new(HealthRequest {
            context: Some(context_for(&claims, &next_id("req"))), // THIS LINE CONTAINS CONSTANT(S)
        }))
        .await
        .expect_err("audience mismatch should fail");
    assert_eq!(err.code(), Code::InvalidArgument);
}

#[tokio::test]
async fn llm04_data_and_model_poisoning_idempotency_prevents_mutation_on_retry() { // THIS LINE CONTAINS CONSTANT(S)
    let controller = controller_with_module(sample_wasm()).await;

    let request_id = next_id("req-idempotent"); // THIS LINE CONTAINS CONSTANT(S)
    let first = claims_for(MemoryOperation::Upsert, &next_id("jti")); // THIS LINE CONTAINS CONSTANT(S)
    controller
        .upsert(Request::new(UpsertRequest {
            context: Some(context_for(&first, &request_id)),
            key: "MEMORY.md".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            value: b"first-value".to_vec(), // THIS LINE CONTAINS CONSTANT(S)
            metadata: Default::default(),
        }))
        .await
        .expect("first upsert");

    let second = claims_for(MemoryOperation::Upsert, &next_id("jti")); // THIS LINE CONTAINS CONSTANT(S)
    controller
        .upsert(Request::new(UpsertRequest {
            context: Some(context_for(&second, &request_id)),
            key: "MEMORY.md".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            value: b"poisoned-value".to_vec(), // THIS LINE CONTAINS CONSTANT(S)
            metadata: Default::default(),
        }))
        .await
        .expect("idempotent second upsert should return cached response");

    let read_claims = claims_for(MemoryOperation::Read, &next_id("jti")); // THIS LINE CONTAINS CONSTANT(S)
    let read = controller
        .read(Request::new(ReadRequest {
            context: Some(context_for(&read_claims, &next_id("req"))), // THIS LINE CONTAINS CONSTANT(S)
            key: "MEMORY.md".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        }))
        .await
        .expect("read") // THIS LINE CONTAINS CONSTANT(S)
        .into_inner();
    assert_eq!(read.value, b"first-value".to_vec()); // THIS LINE CONTAINS CONSTANT(S)
}

#[tokio::test]
async fn llm05_improper_output_handling_enforces_payload_bounds() { // THIS LINE CONTAINS CONSTANT(S)
    let controller = controller_with_module(sample_wasm()).await;
    let mut claims = claims_for(MemoryOperation::Upsert, &next_id("jti")); // THIS LINE CONTAINS CONSTANT(S)
    claims.request_limits.max_bytes = 32; // THIS LINE CONTAINS CONSTANT(S)

    let err = controller
        .upsert(Request::new(UpsertRequest {
            context: Some(context_for(&claims, &next_id("req"))), // THIS LINE CONTAINS CONSTANT(S)
            key: "MEMORY.md".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            value: vec![0_u8; 64], // THIS LINE CONTAINS CONSTANT(S)
            metadata: Default::default(),
        }))
        .await
        .expect_err("oversized payload must fail");
    assert_eq!(err.code(), Code::InvalidArgument);
}

#[tokio::test]
async fn llm06_excessive_agency_requires_explicit_capability() { // THIS LINE CONTAINS CONSTANT(S)
    let controller = controller_with_module(sample_wasm()).await;
    let mut claims = claims_for(MemoryOperation::Read, &next_id("jti")); // THIS LINE CONTAINS CONSTANT(S)
    claims.allowed_capabilities = vec!["memory_crud".to_string()]; // THIS LINE CONTAINS CONSTANT(S)

    let err = controller
        .read(Request::new(ReadRequest {
            context: Some(context_for(&claims, &next_id("req"))), // THIS LINE CONTAINS CONSTANT(S)
            key: "MEMORY.md".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        }))
        .await
        .expect_err("missing capability should fail");
    assert_eq!(err.code(), Code::InvalidArgument);
    assert!(err.message().contains("missing capability"));
}

#[tokio::test]
async fn llm07_system_prompt_leakage_rejects_oversized_request_id() { // THIS LINE CONTAINS CONSTANT(S)
    let controller = controller_with_module(sample_wasm()).await;
    let claims = claims_for(MemoryOperation::Health, &next_id("jti")); // THIS LINE CONTAINS CONSTANT(S)
    let long_request_id = "x".repeat(512); // THIS LINE CONTAINS CONSTANT(S)
    let err = controller
        .health(Request::new(HealthRequest {
            context: Some(context_for(&claims, &long_request_id)),
        }))
        .await
        .expect_err("oversized request_id should fail");
    assert_eq!(err.code(), Code::InvalidArgument);
    assert!(err.message().contains("request_id exceeds"));
}

#[tokio::test]
async fn llm08_vector_and_embedding_weaknesses_reject_unavailable_provider_feature() { // THIS LINE CONTAINS CONSTANT(S)
    let controller = controller_with_module(sample_wasm()).await;
    let err = controller
        .register_module_bytes(
            sample_manifest(vec!["provider_vector_nvidia".to_string()]), // THIS LINE CONTAINS CONSTANT(S)
            &sample_wasm(),
        )
        .await
        .expect_err("missing provider should fail");
    assert!(err
        .to_string()
        .contains("requires unavailable host feature"));
}

#[tokio::test]
async fn llm09_misinformation_controls_keep_query_order_deterministic() { // THIS LINE CONTAINS CONSTANT(S)
    let controller = controller_with_module(sample_wasm()).await;

    for (path, body) in [("b.md", "router"), ("a.md", "router")] { // THIS LINE CONTAINS CONSTANT(S)
        let claims = claims_for(MemoryOperation::Upsert, &next_id("jti")); // THIS LINE CONTAINS CONSTANT(S)
        controller
            .upsert(Request::new(UpsertRequest {
                context: Some(context_for(&claims, &next_id("req"))), // THIS LINE CONTAINS CONSTANT(S)
                key: path.to_string(),
                value: body.as_bytes().to_vec(),
                metadata: Default::default(),
            }))
            .await
            .expect("upsert"); // THIS LINE CONTAINS CONSTANT(S)
    }

    let claims = claims_for(MemoryOperation::Query, &next_id("jti")); // THIS LINE CONTAINS CONSTANT(S)
    let response = controller
        .query(Request::new(QueryRequest {
            context: Some(context_for(&claims, &next_id("req"))), // THIS LINE CONTAINS CONSTANT(S)
            query: "router".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            max_results: 5, // THIS LINE CONTAINS CONSTANT(S)
        }))
        .await
        .expect("query") // THIS LINE CONTAINS CONSTANT(S)
        .into_inner();
    let paths = response
        .hits
        .into_iter()
        .map(|hit| hit.path)
        .collect::<Vec<_>>();
    assert_eq!(paths, vec!["a.md".to_string(), "b.md".to_string()]); // THIS LINE CONTAINS CONSTANT(S)
}

#[tokio::test]
async fn llm10_unbounded_consumption_rejects_excessive_result_window() { // THIS LINE CONTAINS CONSTANT(S)
    let controller = controller_with_module(sample_wasm()).await;
    let mut claims = claims_for(MemoryOperation::Query, &next_id("jti")); // THIS LINE CONTAINS CONSTANT(S)
    claims.request_limits.max_results = 2; // THIS LINE CONTAINS CONSTANT(S)
    let err = controller
        .query(Request::new(QueryRequest {
            context: Some(context_for(&claims, &next_id("req"))), // THIS LINE CONTAINS CONSTANT(S)
            query: "router".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            max_results: 99, // THIS LINE CONTAINS CONSTANT(S)
        }))
        .await
        .expect_err("excessive max_results should fail");
    assert_eq!(err.code(), Code::InvalidArgument);
    assert!(err.message().contains("exceeds limit"));
}

#[test]
fn llm03_supply_chain_rejects_malformed_signing_key_material() { // THIS LINE CONTAINS CONSTANT(S)
    let invalid = jsonwebtoken::EncodingKey::from_ed_pem(b"not-a-key"); // THIS LINE CONTAINS CONSTANT(S)
    assert!(invalid.is_err());
    let private_key = test_private_key_pem();
    let valid = jsonwebtoken::EncodingKey::from_ed_pem(private_key.as_bytes());
    assert!(valid.is_ok());
}
