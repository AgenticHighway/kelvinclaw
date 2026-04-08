# C4 Level 1 — System Context

> Who uses KelvinClaw and what external systems does it interact with?

```mermaid
flowchart TD
    User["👤 End User\n(Developer / Operator)"]
    ChatPlatforms["💬 Chat Platforms\n(Telegram, Slack, Discord, WhatsApp)"]

    KC["🦀 KelvinClaw\n\nSecure, modular harness\nfor agentic AI workflows.\nOrchestrates LLM-powered\nagent runs with WASM-sandboxed\nplugins and policy-driven\nextension loading."]

    LLMProviders["🤖 LLM Providers\n(OpenAI, Anthropic, OpenRouter)"]
    SearchAPIs["🔍 Search APIs\n(Brave Search)"]
    AWSKMS["🔐 AWS KMS\n(EdDSA JWT Signing)"]
    PluginRepo["📦 Plugin Repository\n(GitHub Releases /\nKelvinClaw Registry)"]

    User -->|"CLI prompt / TUI session /\nWebSocket messages"| KC
    ChatPlatforms -->|"HTTP webhooks\n(inbound messages)"| KC
    KC -->|"Stream events /\nresponses"| User
    KC -->|"Webhook replies"| ChatPlatforms
    KC -->|"HTTPS inference\nrequests"| LLMProviders
    KC -->|"HTTPS search\nqueries"| SearchAPIs
    KC -->|"Sign JWT\ndelegation tokens"| AWSKMS
    KC -->|"Fetch plugin\npackages"| PluginRepo
```

## Legend

| Element               | Description                                                                                        |
| --------------------- | -------------------------------------------------------------------------------------------------- |
| **End User**          | Developer or operator interacting via CLI (`kelvin-host`), TUI (`kelvin-tui`), or WebSocket client |
| **Chat Platforms**    | External messaging services that send inbound webhooks to the gateway                              |
| **KelvinClaw**        | The entire KelvinClaw system — gateway, runtime, brain, memory, plugins                            |
| **LLM Providers**     | Third-party model inference APIs accessed through WASM model plugins                               |
| **Search APIs**       | External search services accessed through WASM tool plugins                                        |
| **AWS KMS**           | Key Management Service used for signing memory delegation JWT tokens                               |
| **Plugin Repository** | Source for installable plugin packages (`.tar.gz` with manifests and WASM payloads)                |
