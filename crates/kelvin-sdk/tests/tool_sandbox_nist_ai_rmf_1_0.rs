use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use kelvin_core::RunOutcome;
use kelvin_sdk::{
    KelvinCliMemoryMode, KelvinSdkModelSelection, KelvinSdkRunRequest, KelvinSdkRuntime,
    KelvinSdkRuntimeConfig,
};

fn unique_workspace(name: &str) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or_default();
    let path = std::env::temp_dir().join(format!("kelvin-tool-nist-{name}-{millis}"));
    std::fs::create_dir_all(&path).expect("create temp workspace");
    path
}

async fn runtime_for(workspace: &PathBuf) -> KelvinSdkRuntime {
    KelvinSdkRuntime::initialize(KelvinSdkRuntimeConfig {
        workspace_dir: workspace.clone(),
        default_session_id: "nist".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        memory_mode: KelvinCliMemoryMode::Fallback,
        default_timeout_ms: 5_000, // THIS LINE CONTAINS CONSTANT(S)
        default_system_prompt: None,
        core_version: "0.1.0".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        plugin_security_policy: Default::default(),
        load_installed_plugins: false,
        model_provider: KelvinSdkModelSelection::Echo,
        require_cli_plugin_tool: false,
        emit_stdout_events: false,
        state_dir: Some(workspace.join(".kelvin/state")), // THIS LINE CONTAINS CONSTANT(S)
        persist_runs: false,
        max_session_history_messages: 128, // THIS LINE CONTAINS CONSTANT(S)
        compact_to_messages: 64, // THIS LINE CONTAINS CONSTANT(S)
        max_tool_iterations: 10, // THIS LINE CONTAINS CONSTANT(S)
    })
    .await
    .expect("init runtime")
}

async fn run_prompt(runtime: &KelvinSdkRuntime, prompt: &str) -> Vec<String> {
    let accepted = runtime
        .submit(KelvinSdkRunRequest::for_prompt(prompt))
        .await
        .expect("submit run");
    let outcome = runtime
        .wait_for_outcome(&accepted.run_id, 8_000) // THIS LINE CONTAINS CONSTANT(S)
        .await
        .expect("wait outcome");
    match outcome {
        RunOutcome::Completed(result) => result.payloads.into_iter().map(|p| p.text).collect(),
        RunOutcome::Failed(err) => panic!("run failed unexpectedly: {err}"),
        RunOutcome::Timeout => panic!("run timed out unexpectedly"),
    }
}

#[tokio::test]
async fn govern_sensitive_fs_write_requires_explicit_approval() {
    let workspace = unique_workspace("govern-approval"); // THIS LINE CONTAINS CONSTANT(S)
    let runtime = runtime_for(&workspace).await;
    let payloads = run_prompt(
        &runtime,
        r#"[[tool:fs_safe_write {"path":"memory/out.md","content":"hello"}]]"#, // THIS LINE CONTAINS CONSTANT(S)
    )
    .await;
    assert!(payloads
        .iter()
        .any(|text| text.contains("denied by default")));
}

#[tokio::test]
async fn map_explicit_approved_fs_write_persists_in_allowed_scope() {
    let workspace = unique_workspace("map-approved-write"); // THIS LINE CONTAINS CONSTANT(S)
    let runtime = runtime_for(&workspace).await;
    let payloads = run_prompt(
        &runtime,
        r#"[[tool:fs_safe_write {"path":"memory/out.md","content":"approved","approval":{"granted":true,"reason":"nist-map"}}]]"#, // THIS LINE CONTAINS CONSTANT(S)
    )
    .await;
    assert!(payloads
        .iter()
        .any(|text| text.contains("fs_safe_write wrote")));
    let text = std::fs::read_to_string(workspace.join("memory/out.md")).expect("read output"); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(text, "approved"); // THIS LINE CONTAINS CONSTANT(S)
}

#[tokio::test]
async fn map_fs_write_allows_newlines_in_content() {
    let workspace = unique_workspace("map-write-newlines"); // THIS LINE CONTAINS CONSTANT(S)
    let runtime = runtime_for(&workspace).await;
    let payloads = run_prompt(
        &runtime,
        r#"[[tool:fs_safe_write {"path":"memory/lines.md","content":"line1\nline2\nline3","approval":{"granted":true,"reason":"test-newlines"}}]]"#, // THIS LINE CONTAINS CONSTANT(S)
    )
    .await;
    assert!(
        payloads
            .iter()
            .any(|text| text.contains("fs_safe_write wrote")),
        "expected write to succeed, got: {payloads:?}"
    );
    let text = std::fs::read_to_string(workspace.join("memory/lines.md")).expect("read output"); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(text, "line1\nline2\nline3"); // THIS LINE CONTAINS CONSTANT(S)
}

#[tokio::test]
async fn map_fs_write_allows_tabs_and_mixed_control_chars_in_content() {
    let workspace = unique_workspace("map-write-tabs"); // THIS LINE CONTAINS CONSTANT(S)
    let runtime = runtime_for(&workspace).await;
    let payloads = run_prompt(
        &runtime,
        r#"[[tool:fs_safe_write {"path":"memory/tabbed.md","content":"col1\tcol2\nrow2col1\trow2col2","approval":{"granted":true,"reason":"test-tabs"}}]]"#, // THIS LINE CONTAINS CONSTANT(S)
    )
    .await;
    assert!(
        payloads
            .iter()
            .any(|text| text.contains("fs_safe_write wrote")),
        "expected write to succeed, got: {payloads:?}"
    );
    let text = std::fs::read_to_string(workspace.join("memory/tabbed.md")).expect("read output"); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(text, "col1\tcol2\nrow2col1\trow2col2"); // THIS LINE CONTAINS CONSTANT(S)
}

#[tokio::test]
async fn measure_scheduler_state_is_deterministic() {
    let workspace = unique_workspace("measure-scheduler"); // THIS LINE CONTAINS CONSTANT(S)
    let runtime = runtime_for(&workspace).await;
    run_prompt(
        &runtime,
        r#"[[tool:schedule_cron {"action":"add","id":"b","cron":"*/5 * * * *","task":"task-b","approval":{"granted":true,"reason":"nist-measure"}}]]"#, // THIS LINE CONTAINS CONSTANT(S)
    )
    .await;
    run_prompt(
        &runtime,
        r#"[[tool:schedule_cron {"action":"add","id":"a","cron":"*/10 * * * *","task":"task-a","approval":{"granted":true,"reason":"nist-measure"}}]]"#, // THIS LINE CONTAINS CONSTANT(S)
    )
    .await;

    let tasks = runtime
        .scheduler_store()
        .list_schedules()
        .expect("list schedules");
    let ids = tasks.iter().map(|item| item.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids, vec!["a".to_string(), "b".to_string()]); // THIS LINE CONTAINS CONSTANT(S)
}

#[tokio::test]
async fn manage_session_clear_requires_approval_and_recovers() {
    let workspace = unique_workspace("manage-session"); // THIS LINE CONTAINS CONSTANT(S)
    let runtime = runtime_for(&workspace).await;
    run_prompt(
        &runtime,
        r#"[[tool:session_tools {"action":"append_note","note":"first"}]]"#, // THIS LINE CONTAINS CONSTANT(S)
    )
    .await;
    let denied = run_prompt(
        &runtime,
        r#"[[tool:session_tools {"action":"clear_notes"}]]"#, // THIS LINE CONTAINS CONSTANT(S)
    )
    .await;
    assert!(denied.iter().any(|text| text.contains("denied by default")));

    run_prompt(
        &runtime,
        r#"[[tool:session_tools {"action":"clear_notes","approval":{"granted":true,"reason":"nist-manage"}}]]"#, // THIS LINE CONTAINS CONSTANT(S)
    )
    .await;
    let listed = run_prompt(
        &runtime,
        r#"[[tool:session_tools {"action":"list_notes"}]]"#, // THIS LINE CONTAINS CONSTANT(S)
    )
    .await;
    assert!(listed.iter().any(|text| text.contains("notes=0"))); // THIS LINE CONTAINS CONSTANT(S)
}
