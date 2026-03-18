use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;
use tokio::time;

use kelvin_core::{
    now_ms, AgentEvent, AgentPayload, AgentRunMeta, AgentRunRequest, AgentRunResult, Brain,
    EventSink, KelvinError, KelvinResult, LifecyclePhase, MemorySearchManager, MemorySearchOptions,
    ModelInput, ModelProvider, SessionDescriptor, SessionMessage, SessionStore, ToolCallInput,
    ToolPhase, ToolRegistry,
};

#[derive(Clone)]
pub struct KelvinBrain {
    session_store: Arc<dyn SessionStore>,
    memory: Arc<dyn MemorySearchManager>,
    model: Arc<dyn ModelProvider>,
    tools: Arc<dyn ToolRegistry>,
    events: Arc<dyn EventSink>,
    seq: Arc<AtomicU64>,
}

struct ToolReceipt<'a> {
    run_id: &'a str,
    session_id: &'a str,
    tool_name: &'a str,
    tool_call_id: &'a str,
    result_class: &'a str,
    reason: &'a str,
    latency_ms: u128,
}

impl KelvinBrain {
    pub fn new(
        session_store: Arc<dyn SessionStore>,
        memory: Arc<dyn MemorySearchManager>,
        model: Arc<dyn ModelProvider>,
        tools: Arc<dyn ToolRegistry>,
        events: Arc<dyn EventSink>,
    ) -> Self {
        Self {
            session_store,
            memory,
            model,
            tools,
            events,
            seq: Arc::new(AtomicU64::new(0)),
        }
    }

    fn next_seq(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::SeqCst) + 1
    }

    async fn emit_lifecycle(
        &self,
        run_id: &str,
        phase: LifecyclePhase,
        message: Option<String>,
    ) -> KelvinResult<()> {
        let event = AgentEvent::lifecycle(self.next_seq(), run_id.to_string(), phase, message);
        self.events.emit(event).await
    }

    async fn emit_assistant(
        &self,
        run_id: &str,
        text: &str,
        final_chunk: bool,
    ) -> KelvinResult<()> {
        let event = AgentEvent::assistant(
            self.next_seq(),
            run_id.to_string(),
            text.to_string(),
            final_chunk,
        );
        self.events.emit(event).await
    }

    async fn emit_tool(
        &self,
        run_id: &str,
        tool_name: &str,
        phase: ToolPhase,
        summary: Option<String>,
        output: Option<String>,
    ) -> KelvinResult<()> {
        let event = AgentEvent::tool(
            self.next_seq(),
            run_id.to_string(),
            tool_name.to_string(),
            phase,
            summary,
            output,
        );
        self.events.emit(event).await
    }

    fn emit_tool_receipt(&self, receipt: ToolReceipt<'_>) {
        let line = json!({
            "stream": "tool_receipt",
            "run_id": receipt.run_id,
            "who": {
                "session_id": receipt.session_id,
            },
            "what": {
                "tool_name": receipt.tool_name,
                "tool_call_id": receipt.tool_call_id,
            },
            "why": sanitize_receipt_reason(receipt.reason),
            "result_class": receipt.result_class,
            "latency_ms": receipt.latency_ms,
        });
        println!("{line}");
    }

    async fn run_inner(&self, req: AgentRunRequest) -> KelvinResult<AgentRunResult> {
        if req.prompt.trim().is_empty() {
            return Err(KelvinError::InvalidInput(
                "prompt must not be empty".to_string(),
            ));
        }

        let started_at = now_ms();
        self.emit_lifecycle(&req.run_id, LifecyclePhase::Start, None)
            .await?;

        self.session_store
            .upsert_session(SessionDescriptor {
                session_id: req.session_id.clone(),
                session_key: req.session_key.clone(),
                workspace_dir: req.workspace_dir.clone(),
            })
            .await?;

        self.session_store
            .append_message(&req.session_id, SessionMessage::user(req.prompt.clone()))
            .await?;

        let history = self
            .session_store
            .history(&req.session_id)
            .await?
            .into_iter()
            .map(|message| format!("{:?}: {}", message.role, message.content))
            .collect::<Vec<_>>();

        let memory_query = req
            .memory_query
            .clone()
            .unwrap_or_else(|| req.prompt.clone());
        let memory_hits = self
            .memory
            .search(&memory_query, MemorySearchOptions::default())
            .await
            .unwrap_or_default();
        let memory_snippets = memory_hits
            .iter()
            .map(|item| {
                format!(
                    "{}#{}-{}: {}",
                    item.path, item.start_line, item.end_line, item.snippet
                )
            })
            .collect::<Vec<_>>();

        let system_prompt = req
            .extra_system_prompt
            .clone()
            .unwrap_or_else(|| "KelvinClaw-style Kelvin brain".to_string());

        let model_input = ModelInput {
            run_id: req.run_id.clone(),
            session_id: req.session_id.clone(),
            system_prompt: system_prompt.clone(),
            user_prompt: req.prompt.clone(),
            memory_snippets: memory_snippets.clone(),
            history,
            tools: self.tools.definitions(),
        };

        let model_output = self.model.infer(model_input).await?;
        let mut stop_reason = model_output.stop_reason.clone();
        let tool_calls = model_output.tool_calls;
        let assistant_text = model_output.assistant_text.trim().to_string();

        let mut payloads = Vec::new();
        let ran_tools = !tool_calls.is_empty();

        if !assistant_text.is_empty() && assistant_text != "NO_REPLY" {
            self.emit_assistant(&req.run_id, &assistant_text, true)
                .await?;
            // Only add to payloads if no tools will run; otherwise the followup response replaces this
            if !ran_tools {
                payloads.push(AgentPayload {
                    text: assistant_text.clone(),
                    is_error: false,
                });
            }
        }

        // Append pre-tool assistant text now so session ordering is correct:
        // user → assistant (decided to call tool) → tool → assistant (final)
        if !assistant_text.is_empty() && ran_tools {
            self.session_store
                .append_message(
                    &req.session_id,
                    SessionMessage::assistant(assistant_text.clone()),
                )
                .await?;
        }

        for tool_call in tool_calls {
            let started_tool_at = now_ms();
            self.emit_tool(
                &req.run_id,
                &tool_call.name,
                ToolPhase::Start,
                Some("tool execution started".to_string()),
                None,
            )
            .await?;

            let Some(tool) = self.tools.get(&tool_call.name) else {
                let summary = format!("unknown tool: {}", tool_call.name);
                self.emit_tool(
                    &req.run_id,
                    &tool_call.name,
                    ToolPhase::Error,
                    Some(summary.clone()),
                    None,
                )
                .await?;
                self.emit_tool_receipt(ToolReceipt {
                    run_id: &req.run_id,
                    session_id: &req.session_id,
                    tool_name: &tool_call.name,
                    tool_call_id: &tool_call.id,
                    result_class: "denied",
                    reason: &summary,
                    latency_ms: now_ms().saturating_sub(started_tool_at),
                });
                payloads.push(AgentPayload {
                    text: summary,
                    is_error: true,
                });
                continue;
            };

            let tool_result = tool
                .call(ToolCallInput {
                    run_id: req.run_id.clone(),
                    session_id: req.session_id.clone(),
                    workspace_dir: req.workspace_dir.clone(),
                    arguments: tool_call.arguments.clone(),
                })
                .await;

            let result = match tool_result {
                Ok(result) => result,
                Err(err) => {
                    let summary = format!("tool '{}' failed: {}", tool.name(), err);
                    self.emit_tool(
                        &req.run_id,
                        tool.name(),
                        ToolPhase::Error,
                        Some(summary.clone()),
                        None,
                    )
                    .await?;
                    self.session_store
                        .append_message(
                            &req.session_id,
                            SessionMessage::tool(
                                format!("{}\n\nError:\n{}", summary, err),
                                json!({
                                    "tool": tool.name(),
                                    "is_error": true,
                                    "error": err.to_string(),
                                }),
                            ),
                        )
                        .await?;
                    self.emit_tool_receipt(ToolReceipt {
                        run_id: &req.run_id,
                        session_id: &req.session_id,
                        tool_name: tool.name(),
                        tool_call_id: &tool_call.id,
                        result_class: "error",
                        reason: &summary,
                        latency_ms: now_ms().saturating_sub(started_tool_at),
                    });
                    payloads.push(AgentPayload {
                        text: summary,
                        is_error: true,
                    });
                    continue;
                }
            };

            let phase = if result.is_error {
                ToolPhase::Error
            } else {
                ToolPhase::End
            };
            self.emit_tool(
                &req.run_id,
                tool.name(),
                phase,
                Some(result.summary.clone()),
                result.output.clone(),
            )
            .await?;

            let tool_content = match &result.output {
                Some(output) => format!("{}\n\nOutput:\n{}", result.summary, output),
                None => result.summary.clone(),
            };
            self.session_store
                .append_message(
                    &req.session_id,
                    SessionMessage::tool(
                        tool_content,
                        json!({
                            "tool": tool.name(),
                            "is_error": result.is_error,
                            "output": result.output,
                        }),
                    ),
                )
                .await?;

            self.emit_tool_receipt(ToolReceipt {
                run_id: &req.run_id,
                session_id: &req.session_id,
                tool_name: tool.name(),
                tool_call_id: &tool_call.id,
                result_class: if result.is_error {
                    "tool_error"
                } else {
                    "success"
                },
                reason: &result.summary,
                latency_ms: now_ms().saturating_sub(started_tool_at),
            });

            if let Some(visible_text) = result.visible_text {
                payloads.push(AgentPayload {
                    text: visible_text,
                    is_error: result.is_error,
                });
            }
        }

        if ran_tools {
            // Re-fetch history now that tool results are appended, then ask the model
            // to interpret the results and produce a final response.
            let updated_history = self
                .session_store
                .history(&req.session_id)
                .await?
                .into_iter()
                .map(|message| format!("{:?}: {}", message.role, message.content))
                .collect::<Vec<_>>();

            let followup_input = ModelInput {
                run_id: req.run_id.clone(),
                session_id: req.session_id.clone(),
                system_prompt: system_prompt.clone(),
                user_prompt: "Tool calls completed. Based on the results in the conversation history above, respond to the user's original request.".to_string(),
                memory_snippets,
                history: updated_history,
                tools: self.tools.definitions(),
            };

            let followup_output = self.model.infer(followup_input).await?;
            stop_reason = followup_output.stop_reason.clone();
            let followup_text = followup_output.assistant_text.trim().to_string();

            if !followup_text.is_empty() && followup_text != "NO_REPLY" {
                self.emit_assistant(&req.run_id, &followup_text, true)
                    .await?;
                payloads.push(AgentPayload {
                    text: followup_text.clone(),
                    is_error: false,
                });
                self.session_store
                    .append_message(
                        &req.session_id,
                        SessionMessage::assistant(followup_text),
                    )
                    .await?;
            }
        } else if !assistant_text.is_empty() {
            // No tools ran: the first response is the final one, save it to session now.
            self.session_store
                .append_message(&req.session_id, SessionMessage::assistant(assistant_text))
                .await?;
        }

        self.emit_lifecycle(&req.run_id, LifecyclePhase::End, None)
            .await?;

        let duration_ms = now_ms().saturating_sub(started_at);
        Ok(AgentRunResult {
            payloads,
            meta: AgentRunMeta {
                duration_ms,
                provider: self.model.provider_name().to_string(),
                model: self.model.model_name().to_string(),
                stop_reason,
                error: None,
            },
        })
    }
}

fn sanitize_receipt_reason(reason: &str) -> String {
    let mut out = String::new();
    for (idx, ch) in reason.chars().enumerate() {
        if idx >= 512 {
            out.push_str("...");
            break;
        }
        if ch.is_control() {
            continue;
        }
        out.push(ch);
    }
    out
}

#[async_trait]
impl Brain for KelvinBrain {
    async fn run(&self, req: AgentRunRequest) -> KelvinResult<AgentRunResult> {
        let run_id = req.run_id.clone();
        let result = match req.timeout_ms {
            Some(timeout_ms) => {
                match time::timeout(Duration::from_millis(timeout_ms), self.run_inner(req)).await {
                    Ok(inner_result) => inner_result,
                    Err(_) => Err(KelvinError::Timeout(format!(
                        "agent run exceeded timeout of {timeout_ms}ms"
                    ))),
                }
            }
            None => self.run_inner(req).await,
        };

        if let Err(err) = &result {
            let _ = self
                .emit_lifecycle(&run_id, LifecyclePhase::Error, Some(err.to_string()))
                .await;
        }

        result
    }
}
