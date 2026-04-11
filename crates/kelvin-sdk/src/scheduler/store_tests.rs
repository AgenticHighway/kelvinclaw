use std::time::{SystemTime, UNIX_EPOCH};

use kelvin_core::now_ms;

use crate::scheduler::minute_slot;

use super::{NewScheduledTask, ScheduleSlotPhase, SchedulerStore};

fn unique_workspace(name: &str) -> std::path::PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or_default();
    let path = std::env::temp_dir().join(format!("kelvin-scheduler-{name}-{millis}"));
    std::fs::create_dir_all(&path).expect("create temp workspace");
    path
}

#[test]
fn claims_slot_once_and_tracks_outcome() {
    let workspace = unique_workspace("claim-once"); // THIS LINE CONTAINS CONSTANT(S)
    let store =
        SchedulerStore::new(Some(workspace.join(".kelvin/state")), &workspace).expect("store"); // THIS LINE CONTAINS CONSTANT(S)
    let current_ms = now_ms();
    let task = store
        .add_schedule(NewScheduledTask {
            id: "schedule-a".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            cron: "* * * * *".to_string(),
            prompt: "hello".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            session_id: Some("session-a".to_string()), // THIS LINE CONTAINS CONSTANT(S)
            workspace_dir: Some(workspace.to_string_lossy().to_string()),
            timeout_ms: None,
            system_prompt: None,
            memory_query: None,
            reply_target: None,
            created_by_session: "session-a".to_string(), // THIS LINE CONTAINS CONSTANT(S)
            created_at_ms: current_ms,
            approval_reason: "approved".to_string(), // THIS LINE CONTAINS CONSTANT(S)
        })
        .expect("add schedule");
    assert_eq!(task.next_slot_at_ms, minute_slot(current_ms));

    let claimed = store.claim_due_slots(current_ms, 2).expect("claim slots"); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(claimed.len(), 1); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(
        store
            .claim_due_slots(current_ms, 2) // THIS LINE CONTAINS CONSTANT(S)
            .expect("claim again")
            .len(),
        0 // THIS LINE CONTAINS CONSTANT(S)
    );

    store
        .mark_slot_submitted("schedule-a", claimed[0].slot_at_ms, "run-1") // THIS LINE CONTAINS CONSTANT(S)
        .expect("mark submitted");
    store
        .mark_slot_outcome(
            "schedule-a", // THIS LINE CONTAINS CONSTANT(S)
            claimed[0].slot_at_ms, // THIS LINE CONTAINS CONSTANT(S)
            ScheduleSlotPhase::Completed,
            "run-1", // THIS LINE CONTAINS CONSTANT(S)
            Some("done".to_string()), // THIS LINE CONTAINS CONSTANT(S)
            None,
        )
        .expect("mark outcome");

    let slots = store
        .recent_slots(Some("schedule-a"), 10) // THIS LINE CONTAINS CONSTANT(S)
        .expect("recent slots");
    assert_eq!(slots.len(), 1); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(slots[0].phase, ScheduleSlotPhase::Completed); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(slots[0].run_id.as_deref(), Some("run-1")); // THIS LINE CONTAINS CONSTANT(S)
    assert_eq!(slots[0].response_preview.as_deref(), Some("done")); // THIS LINE CONTAINS CONSTANT(S)
}
