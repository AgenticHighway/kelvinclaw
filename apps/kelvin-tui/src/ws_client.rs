use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

use crate::app::{AgentEvent, TuiEvent, WsStatus};

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientFrame {
    Req {
        id: String,
        method: String,
        params: Value,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerFrame {
    Res {
        id: String,
        ok: bool,
        payload: Option<Value>,
        error: Option<Value>,
    },
    Event {
        event: String,
        payload: Value,
    },
}

type PendingMap = Arc<Mutex<HashMap<String, oneshot::Sender<Result<Value, String>>>>>;

#[derive(Clone)]
pub struct WsClient {
    sender: mpsc::Sender<String>,
    pending: PendingMap,
}

impl WsClient {
    pub async fn connect(
        url: &str,
        auth_token: Option<String>,
        tui_tx: mpsc::Sender<TuiEvent>,
    ) -> Result<Self, String> {
        let (ws_stream, _) = timeout(Duration::from_secs(10), connect_async(url))
            .await
            .map_err(|_| "WebSocket connect timed out".to_string())?
            .map_err(|e| format!("WebSocket connect failed: {e}"))?;

        let (mut ws_write, mut ws_read) = ws_stream.split();

        let (frame_tx, mut frame_rx) = mpsc::channel::<String>(128);
        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
        let pending_clone = pending.clone();
        let tui_tx_clone = tui_tx.clone();
        let tui_tx_writer = tui_tx.clone();

        tokio::spawn(async move {
            while let Some(msg) = frame_rx.recv().await {
                if ws_write.send(Message::Text(msg.into())).await.is_err() {
                    let _ = tui_tx_writer
                        .send(TuiEvent::WsStatus(WsStatus::Disconnected))
                        .await;
                    break;
                }
            }
        });

        tokio::spawn(async move {
            while let Some(msg) = ws_read.next().await {
                match msg {
                    Ok(Message::Text(text)) => match serde_json::from_str::<ServerFrame>(&text) {
                        Ok(ServerFrame::Res {
                            id,
                            ok,
                            payload,
                            error,
                        }) => {
                            let result = if ok {
                                Ok(payload.unwrap_or(Value::Null))
                            } else {
                                Err(error
                                    .and_then(|e| {
                                        e.get("message")
                                            .and_then(|m| m.as_str())
                                            .map(|s| s.to_string())
                                    })
                                    .unwrap_or_else(|| "unknown error".to_string()))
                            };
                            let mut map = pending_clone.lock().await;
                            if let Some(tx) = map.remove(&id) {
                                let _ = tx.send(result);
                            }
                        }
                        Ok(ServerFrame::Event { event, payload }) => {
                            if event == "agent" {
                                match serde_json::from_value::<AgentEvent>(payload) {
                                    Ok(ev) => {
                                        let _ = tui_tx_clone.send(TuiEvent::Agent(ev)).await;
                                    }
                                    Err(e) => {
                                        let _ = tui_tx_clone
                                            .send(TuiEvent::WsStatus(WsStatus::Error(format!(
                                                "failed to parse agent event: {e}"
                                            ))))
                                            .await;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            let _ = tui_tx_clone
                                .send(TuiEvent::WsStatus(WsStatus::Error(format!(
                                    "failed to parse server frame: {e}"
                                ))))
                                .await;
                        }
                    },
                    Ok(Message::Close(_)) | Err(_) => {
                        let _ = tui_tx_clone
                            .send(TuiEvent::WsStatus(WsStatus::Disconnected))
                            .await;
                        break;
                    }
                    _ => {}
                }
            }
        });

        let client = WsClient {
            sender: frame_tx,
            pending,
        };

        let connect_params = if let Some(token) = auth_token {
            json!({ "auth": { "token": token }, "client_id": "kelvin-tui" })
        } else {
            json!({ "client_id": "kelvin-tui" })
        };

        client.call("connect", connect_params).await?;
        Ok(client)
    }

    async fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = Uuid::new_v4().to_string();
        let frame = ClientFrame::Req {
            id: id.clone(),
            method: method.to_string(),
            params,
        };
        let text = serde_json::to_string(&frame).map_err(|e| e.to_string())?;

        let (tx, rx) = oneshot::channel();
        let id_for_cleanup = id.clone();
        {
            let mut map = self.pending.lock().await;
            map.insert(id, tx);
        }

        self.sender
            .send(text)
            .await
            .map_err(|_| "sender closed".to_string())?;

        match timeout(Duration::from_secs(30), rx).await {
            Ok(result) => result.map_err(|_| "response channel closed".to_string())?,
            Err(_) => {
                self.pending.lock().await.remove(&id_for_cleanup);
                return Err(format!("request '{method}' timed out"));
            }
        }
    }

    pub async fn submit_prompt(&self, prompt: &str, session_id: &str) -> Result<String, String> {
        let request_id = Uuid::new_v4().to_string();
        let params = json!({
            "request_id": request_id,
            "prompt": prompt,
            "session_id": session_id,
        });
        let result = self.call("run.submit", params).await?;
        result
            .get("run_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "missing run_id in response".to_string())
    }

    pub async fn list_commands(&self) -> Result<Value, String> {
        self.call("commands.list", json!({})).await
    }

    pub async fn exec_command(
        &self,
        command: &str,
        args: Value,
        session_id: &str,
    ) -> Result<Value, String> {
        self.call(
            "command.exec",
            json!({
                "command": command,
                "args": args,
                "session_id": session_id,
            }),
        )
        .await
    }

    #[allow(dead_code)]
    pub async fn health(&self) -> Result<Value, String> {
        self.call("health", json!({})).await
    }
}
