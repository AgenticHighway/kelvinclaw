# C4 Level 2 — Container Diagram

> What are the deployable units and how do they communicate?

## Container Overview

```mermaid
flowchart TD
    subgraph External["External Actors"]
        User["👤 End User"]
        Bots["💬 Chat Platforms\n(Telegram/Slack/Discord/WhatsApp)"]
        LLMs["🤖 LLM Providers\n(OpenAI/Anthropic/OpenRouter)"]
        Search["🔍 Search APIs"]
        DeepWiki["📚 DeepWiki MCP API"]
        KMS["🔐 AWS KMS"]
    end

    subgraph KelvinClaw["KelvinClaw System"]
        Gateway["kelvin-gateway\n\nWebSocket server +\nHTTP ingress gateway\n(Rust / Tokio)"]
        Host["kelvin-host\n\nCLI agent runner\n(Rust binary)"]
        TUI["kelvin-tui\n\nTerminal UI client\n(Rust / Ratatui)"]
        Registry["kelvin-registry\n\nPlugin discovery\nHTTP service\n(Rust / Axum)"]
        MemCtrl["kelvin-memory-controller\n\ngRPC memory data plane\n(Rust / Tonic)"]

        SDK["kelvin-sdk\n\nComposition + wiring layer\n(Rust library)"]
        Brain["kelvin-brain\n\nAgent loop orchestrator\n(Rust library)"]
        WASM["kelvin-wasm\n\nWASM sandbox host\n(Rust / Wasmtime)"]
        Core["kelvin-core\n\nDomain traits + contracts\n(Rust library)"]
        MemClient["kelvin-memory-client\n\ngRPC memory client\n(Rust library)"]
    end

    User -->|"stdin/stdout"| Host
    User -->|"WebSocket\n:34617"| TUI
    TUI -->|"WebSocket\n:34617"| Gateway
    Bots -->|"HTTP webhooks\n:34618"| Gateway

    Gateway -->|uses| SDK
    Host -->|uses| SDK
    SDK -->|orchestrates| Brain
    SDK -->|wires| MemClient
    Brain -->|loads plugins via| WASM
    Brain -->|implements| Core
    WASM -->|host-mediated HTTPS| LLMs
    WASM -->|host-mediated HTTPS| Search
    WASM -->|host-mediated HTTPS| DeepWiki
    MemClient -->|"gRPC :50051"| MemCtrl
    MemClient -->|"sign JWTs"| KMS
    MemCtrl -->|"executes memory\nWASM modules"| WASM

    Host -->|"HTTP :34619"| Registry
    Gateway -->|"HTTP :34619"| Registry
```

## Container Descriptions

| Container                    | Technology                 | Port(s)                    | Purpose                                                                                                                                                                                   |
| ---------------------------- | -------------------------- | -------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **kelvin-gateway**           | Rust / Tokio / Tungstenite | `:34617` WS, `:34618` HTTP | Primary long-running service. WebSocket API for clients, HTTP ingress for chat platform webhooks. Manages sessions, scheduling, idempotency.                                              |
| **kelvin-host**              | Rust CLI binary            | —                          | Thin CLI for single-prompt or interactive agent runs. Direct user-facing.                                                                                                                 |
| **kelvin-tui**               | Rust / Ratatui             | —                          | Terminal UI that connects to gateway over WebSocket.                                                                                                                                      |
| **kelvin-registry**          | Rust / Axum HTTP           | `:34619`                   | Optional plugin discovery and index query service.                                                                                                                                        |
| **kelvin-memory-controller** | Rust / Tonic gRPC          | `:50051`                   | Memory data plane. Validates JWT delegation tokens, executes WASM memory modules, replay protection.                                                                                      |
| **kelvin-sdk**               | Rust library               | —                          | Composition layer wiring brain, memory, plugins, sessions, and runtime config. Includes ToolPack built-in tools (fs read/write, web fetch, scheduler, session). Used by gateway and host. |
| **kelvin-brain**             | Rust library               | —                          | Agent loop: prompt → model → tool → persist. Plugin loading and execution.                                                                                                                |
| **kelvin-wasm**              | Rust / Wasmtime            | —                          | Trusted WASM sandbox host. Executes untrusted model, tool, channel, and memory plugins.                                                                                                   |
| **kelvin-core**              | Rust library               | —                          | Pure domain models and trait contracts. Zero external dependencies. The stable API surface.                                                                                               |
| **kelvin-memory-client**     | Rust library               | —                          | gRPC client implementing `MemorySearchManager`. Mints JWT delegation tokens.                                                                                                              |

## Communication Protocols

```mermaid
sequenceDiagram
    participant U as End User
    participant T as kelvin-tui
    participant G as kelvin-gateway
    participant S as kelvin-sdk
    participant B as kelvin-brain
    participant W as kelvin-wasm
    participant L as LLM Provider
    participant MC as kelvin-memory-client
    participant MD as kelvin-memory-controller

    U->>T: User types prompt
    T->>G: WebSocket: {type: "request", method: "agent", params: {...}}
    G->>S: submit_run(request)
    S->>B: KelvinBrain.run()
    B->>MC: memory recall (search query)
    MC->>MD: gRPC: Query(jwt, query)
    MD-->>MC: MemorySearchResult[]
    MC-->>B: context fragments
    B->>W: ModelProvider.infer() via WASM
    W->>L: HTTPS POST /v1/chat/completions
    L-->>W: model response
    W-->>B: ModelOutput
    B-->>S: AgentRunResult
    S-->>G: run outcome
    G-->>T: WebSocket: stream events
    T-->>U: Display response
```
