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
    ModelInput, ModelProvider, SessionDescriptor, SessionMessage, SessionStore, ToolCall,
    ToolCallInput, ToolPhase, ToolRegistry,
};

#[derive(Clone)]
pub struct KelvinBrain {
    session_store: Arc<dyn SessionStore>,
    memory: Arc<dyn MemorySearchManager>,
    model: Arc<dyn ModelProvider>,
    tools: Arc<dyn ToolRegistry>,
    events: Arc<dyn EventSink>,
    seq: Arc<AtomicU64>,
    max_tool_iterations: usize,
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
            max_tool_iterations: 10,
        }
    }

    pub fn with_max_tool_iterations(mut self, max: usize) -> Self {
        self.max_tool_iterations = max;
        self
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

    async fn execute_tool_calls(
        &self,
        req: &AgentRunRequest,
        tool_calls: &[ToolCall],
    ) -> KelvinResult<Vec<AgentPayload>> {
        let mut payloads = Vec::new();

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

        Ok(payloads)
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
            .unwrap_or_else(|| {
                "KelvinClaw-style Kelvin brain\n\nIMPORTANT: Do not call the same tool multiple times with identical or nearly identical inputs. If a tool call did not achieve the desired outcome, either try a different tool, modify your approach, or ask the user for clarification instead of retrying."
                    .to_string()
            });

        let max_iter = req.max_tool_iterations.unwrap_or(self.max_tool_iterations);
        let mut iteration = 0usize;
        let mut payloads: Vec<AgentPayload> = Vec::new();
        #[allow(unused_assignments)]
        let mut stop_reason: Option<String> = None;

        loop {
            let history = self
                .session_store
                .history(&req.session_id)
                .await?
                .into_iter()
                .map(|message| format!("{:?}: {}", message.role, message.content))
                .collect::<Vec<_>>();

            let user_prompt = if iteration == 0 {
                req.prompt.clone()
            } else {
                "Tool calls completed. Based on the results in the conversation history above, respond to the user's original request.".to_string()
            };

            let model_input = ModelInput {
                run_id: req.run_id.clone(),
                session_id: req.session_id.clone(),
                system_prompt: system_prompt.clone(),
                user_prompt,
                memory_snippets: memory_snippets.clone(),
                history,
                tools: self.tools.definitions(),
            };

            let output = self.model.infer(model_input).await?;
            stop_reason = output.stop_reason.clone();
            let text = output.assistant_text.trim().to_string();
            let has_tools = !output.tool_calls.is_empty();

            if !text.is_empty() && text != "NO_REPLY" {
                self.emit_assistant(&req.run_id, &text, !has_tools).await?;
                self.session_store
                    .append_message(
                        &req.session_id,
                        SessionMessage::assistant(text.clone()),
                    )
                    .await?;
                if !has_tools {
                    payloads.push(AgentPayload {
                        text,
                        is_error: false,
                    });
                }
            }

            if !has_tools {
                break;
            }

            let tool_payloads = self.execute_tool_calls(&req, &output.tool_calls).await?;
            payloads.extend(tool_payloads);

            iteration += 1;
            if iteration >= max_iter {
                self.emit_lifecycle(
                    &req.run_id,
                    LifecyclePhase::Warning,
                    Some(format!("max tool iterations ({max_iter}) reached")),
                )
                .await?;

                let final_history = self
                    .session_store
                    .history(&req.session_id)
                    .await?
                    .into_iter()
                    .map(|message| format!("{:?}: {}", message.role, message.content))
                    .collect::<Vec<_>>();

                let final_input = ModelInput {
                    run_id: req.run_id.clone(),
                    session_id: req.session_id.clone(),
                    system_prompt: system_prompt.clone(),
                    user_prompt: "Tool calls completed. Based on the results in the conversation history above, respond to the user's original request.".to_string(),
                    memory_snippets: memory_snippets.clone(),
                    history: final_history,
                    tools: vec![],
                };

                let final_output = self.model.infer(final_input).await?;
                stop_reason = final_output.stop_reason.clone();
                let final_text = final_output.assistant_text.trim().to_string();

                if !final_text.is_empty() && final_text != "NO_REPLY" {
                    self.emit_assistant(&req.run_id, &final_text, true).await?;
                    self.session_store
                        .append_message(
                            &req.session_id,
                            SessionMessage::assistant(final_text.clone()),
                        )
                        .await?;
                    payloads.push(AgentPayload {
                        text: final_text,
                        is_error: false,
                    });
                }
                break;
            }
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
                tool_iterations: iteration,
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
