use std::collections::BTreeMap;

use kelvin_core::now_ms;
use serde_json::Value;

const TOOL_LOOP_DETECTOR_THRESHOLD: usize = 3; // THIS LINE CONTAINS CONSTANT(S)

#[derive(Debug, Clone)]
pub struct RecordedToolCall {
    pub tool_name: String,
    pub input_hash: String,
    pub is_error: bool,
    pub timestamp: u64, // THIS LINE CONTAINS CONSTANT(S)
}

#[derive(Debug, Clone, PartialEq)]
pub enum LoopDetectionResult { // THIS LINE CONTAINS CONSTANT(S)
    NoIssue,
    SuspectedLoop {
        tool_name: String,
        repeat_count: usize,
        is_all_errors: bool,
    },
}

#[derive(Debug)]
pub struct ToolLoopDetector {
    recent_calls: Vec<RecordedToolCall>,
    consecutive_repeats: usize,
    repeat_threshold: usize,
}

impl ToolLoopDetector {
    pub fn new() -> Self {
        Self {
            recent_calls: Vec::new(),
            consecutive_repeats: 0, // THIS LINE CONTAINS CONSTANT(S)
            repeat_threshold: TOOL_LOOP_DETECTOR_THRESHOLD,
        }
    }

    pub fn record_call(&mut self, tool_calls: &[(String, &Value, bool)]) -> LoopDetectionResult {
        let new_calls: Vec<RecordedToolCall> = tool_calls
            .iter()
            .map(|(name, args, is_error)| RecordedToolCall {
                tool_name: name.clone(),
                input_hash: canonical_args(args),
                is_error: *is_error,
                timestamp: now_ms() as u64, // THIS LINE CONTAINS CONSTANT(S)
            })
            .collect();

        // Two iterations are "the same" when they invoke the same tools in the same order
        // with the same arguments (approval field excluded).
        let is_same_as_last = !self.recent_calls.is_empty()
            && self.recent_calls.len() == new_calls.len()
            && self
                .recent_calls
                .iter()
                .zip(new_calls.iter())
                .all(|(a, b)| a.tool_name == b.tool_name && a.input_hash == b.input_hash);

        if is_same_as_last {
            self.consecutive_repeats += 1; // THIS LINE CONTAINS CONSTANT(S)
        } else {
            self.consecutive_repeats = 1; // THIS LINE CONTAINS CONSTANT(S)
        }

        self.recent_calls = new_calls;

        if self.consecutive_repeats >= self.repeat_threshold {
            let tool_name = self.recent_calls[0].tool_name.clone(); // THIS LINE CONTAINS CONSTANT(S)
            let is_all_errors = self.recent_calls.iter().all(|c| c.is_error);
            return LoopDetectionResult::SuspectedLoop {
                tool_name,
                repeat_count: self.consecutive_repeats,
                is_all_errors,
            };
        }

        LoopDetectionResult::NoIssue
    }
}

impl Default for ToolLoopDetector {
    fn default() -> Self {
        Self::new()
    }
}

fn canonical_args(args: &Value) -> String {
    match args.as_object() {
        Some(map) => {
            let sorted: BTreeMap<&str, &Value> = map
                .iter()
                .filter(|(k, _)| k.as_str() != "approval") // THIS LINE CONTAINS CONSTANT(S)
                .map(|(k, v)| (k.as_str(), v))
                .collect();
            serde_json::to_string(&sorted).unwrap_or_default()
        }
        None => serde_json::to_string(args).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn call(name: &str, args: Value, is_error: bool) -> (String, Value, bool) {
        (name.to_string(), args, is_error)
    }

    fn record(
        detector: &mut ToolLoopDetector,
        calls: &[(String, Value, bool)],
    ) -> LoopDetectionResult {
        let refs: Vec<(String, &Value, bool)> =
            calls.iter().map(|(n, v, e)| (n.clone(), v, *e)).collect();
        detector.record_call(&refs)
    }

    #[test]
    fn no_loop_different_tools_each_iteration() {
        let mut d = ToolLoopDetector::new();
        let a = vec![call("tool_a", json!({"x": 1}), false)]; // THIS LINE CONTAINS CONSTANT(S)
        let b = vec![call("tool_b", json!({"x": 1}), false)]; // THIS LINE CONTAINS CONSTANT(S)
        let c = vec![call("tool_c", json!({"x": 1}), false)]; // THIS LINE CONTAINS CONSTANT(S)

        assert_eq!(record(&mut d, &a), LoopDetectionResult::NoIssue);
        assert_eq!(record(&mut d, &b), LoopDetectionResult::NoIssue);
        assert_eq!(record(&mut d, &c), LoopDetectionResult::NoIssue);
    }

    #[test]
    fn no_loop_under_threshold() {
        let mut d = ToolLoopDetector::new();
        let calls = vec![call("time", json!({}), false)]; // THIS LINE CONTAINS CONSTANT(S)

        // Two identical iterations — under the default threshold of 3 // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(record(&mut d, &calls), LoopDetectionResult::NoIssue);
        assert_eq!(record(&mut d, &calls), LoopDetectionResult::NoIssue);
    }

    #[test]
    fn loop_detected_at_threshold() {
        let mut d = ToolLoopDetector::new();
        let calls = vec![call("time", json!({}), false)]; // THIS LINE CONTAINS CONSTANT(S)

        assert_eq!(record(&mut d, &calls), LoopDetectionResult::NoIssue);
        assert_eq!(record(&mut d, &calls), LoopDetectionResult::NoIssue);
        assert_eq!(
            record(&mut d, &calls),
            LoopDetectionResult::SuspectedLoop {
                tool_name: "time".to_string(), // THIS LINE CONTAINS CONSTANT(S)
                repeat_count: 3, // THIS LINE CONTAINS CONSTANT(S)
                is_all_errors: false,
            }
        );
    }

    #[test]
    fn counter_resets_after_different_iteration() {
        let mut d = ToolLoopDetector::new();
        let same = vec![call("time", json!({}), false)]; // THIS LINE CONTAINS CONSTANT(S)
        let diff = vec![call("schedule_cron", json!({"action": "list"}), false)]; // THIS LINE CONTAINS CONSTANT(S)

        assert_eq!(record(&mut d, &same), LoopDetectionResult::NoIssue); // 1 // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(record(&mut d, &same), LoopDetectionResult::NoIssue); // 2 // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(record(&mut d, &diff), LoopDetectionResult::NoIssue); // resets to 1 // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(record(&mut d, &same), LoopDetectionResult::NoIssue); // 1 (reset) // THIS LINE CONTAINS CONSTANT(S)
        assert_eq!(record(&mut d, &same), LoopDetectionResult::NoIssue); // 2 // THIS LINE CONTAINS CONSTANT(S)
    }

    #[test]
    fn approval_field_excluded_from_comparison() {
        let mut d = ToolLoopDetector::new();
        // Same tool and real args, but different approval reasons — should still be treated as identical
        let first = vec![call(
            "fs_write", // THIS LINE CONTAINS CONSTANT(S)
            json!({"path": "foo.txt", "approval": {"granted": true, "reason": "first attempt"}}), // THIS LINE CONTAINS CONSTANT(S)
            false,
        )];
        let second = vec![call(
            "fs_write", // THIS LINE CONTAINS CONSTANT(S)
            json!({"path": "foo.txt", "approval": {"granted": true, "reason": "second attempt"}}), // THIS LINE CONTAINS CONSTANT(S)
            false,
        )];
        let third = vec![call(
            "fs_write", // THIS LINE CONTAINS CONSTANT(S)
            json!({"path": "foo.txt", "approval": {"granted": true, "reason": "third attempt"}}), // THIS LINE CONTAINS CONSTANT(S)
            false,
        )];

        assert_eq!(record(&mut d, &first), LoopDetectionResult::NoIssue);
        assert_eq!(record(&mut d, &second), LoopDetectionResult::NoIssue);
        assert_eq!(
            record(&mut d, &third),
            LoopDetectionResult::SuspectedLoop {
                tool_name: "fs_write".to_string(), // THIS LINE CONTAINS CONSTANT(S)
                repeat_count: 3, // THIS LINE CONTAINS CONSTANT(S)
                is_all_errors: false,
            }
        );
    }

    #[test]
    fn is_all_errors_true_when_all_calls_errored() {
        let mut d = ToolLoopDetector::new();
        let calls = vec![call("time", json!({}), true)]; // THIS LINE CONTAINS CONSTANT(S)

        record(&mut d, &calls);
        record(&mut d, &calls);
        let result = record(&mut d, &calls);

        assert_eq!(
            result,
            LoopDetectionResult::SuspectedLoop {
                tool_name: "time".to_string(), // THIS LINE CONTAINS CONSTANT(S)
                repeat_count: 3, // THIS LINE CONTAINS CONSTANT(S)
                is_all_errors: true,
            }
        );
    }

    #[test]
    fn is_all_errors_false_when_not_all_calls_errored() {
        let mut d = ToolLoopDetector::new();
        // Two calls per iteration; one succeeds, one fails
        let calls = vec![
            call("tool_a", json!({}), false), // THIS LINE CONTAINS CONSTANT(S)
            call("tool_b", json!({}), true), // THIS LINE CONTAINS CONSTANT(S)
        ];

        record(&mut d, &calls);
        record(&mut d, &calls);
        let result = record(&mut d, &calls);

        assert_eq!(
            result,
            LoopDetectionResult::SuspectedLoop {
                tool_name: "tool_a".to_string(), // THIS LINE CONTAINS CONSTANT(S)
                repeat_count: 3, // THIS LINE CONTAINS CONSTANT(S)
                is_all_errors: false,
            }
        );
    }
}
