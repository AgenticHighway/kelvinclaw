# C4 Level 4 — Code Diagrams

> Key code-level flows and type relationships.

## Agent Run Execution Flow

```mermaid
sequenceDiagram
    participant Client as Client (TUI / CLI / WS)
    participant GW as kelvin-gateway
    participant SDK as KelvinSdkRuntime
    participant CR as CoreRuntime
    participant RR as RunRegistry
    participant KB as KelvinBrain
    participant SS as SessionStore
    participant MS as MemorySearchManager
    participant MP as ModelProvider (WASM)
    participant TL as Tool (WASM)
    participant ES as EventSink

    Client->>GW: {method: "agent", params: {prompt, session_id}}
    GW->>SDK: submit_run(AgentRunRequest)
    SDK->>CR: register_run()
    CR->>RR: insert(run_id, RunState::Pending)
    CR-->>SDK: run_id
    SDK->>KB: run(AgentRunRequest)

    KB->>ES: emit(LifecycleStart)
    KB->>SS: upsert_session(session_id)
    KB->>SS: append(UserMessage)

    KB->>SS: get_history(session_id)
    SS-->>KB: Vec of SessionMessage

    KB->>MS: search(query, opts)
    MS-->>KB: Vec of MemorySearchResult

    KB->>MP: infer(ModelInput{history, context, tools})
    MP-->>KB: ModelOutput{content, tool_calls}

    alt Tool calls present
        loop For each tool_call
            KB->>TL: call(ToolCallInput)
            TL-->>KB: ToolCallResult
            KB->>SS: append(ToolMessage)
            KB->>ES: emit(ToolResult)
        end
        KB->>MP: infer(ModelInput{...with tool results})
        MP-->>KB: ModelOutput{content}
    end

    KB->>SS: append(AssistantMessage)
    KB->>ES: emit(AssistantMessage)
    KB->>ES: emit(LifecycleEnd)
    KB-->>SDK: AgentRunResult
    SDK->>RR: update(run_id, RunState::Complete, RunOutcome)
    SDK-->>GW: AgentRunResult
    GW-->>Client: stream events
```

## Plugin Loading Flow

```mermaid
flowchart TD
    Start["Plugin Home Dir\n~/.kelvinclaw/plugins/"] --> Scan["Scan directories\nfor plugin.json manifests"]
    Scan --> Parse["Parse PluginManifest\nid, version, abi, capabilities"]
    Parse --> SigCheck{"Signature\nplugin.sig\npresent?"}

    SigCheck -->|Yes| Verify["Verify Ed25519 signature\nagainst trusted_publishers.json"]
    SigCheck -->|No| TrustPolicy{"Trust policy\nrequires\nsignatures?"}

    Verify -->|Valid| Caps["Validate capabilities\nagainst SecurityPolicy"]
    Verify -->|Invalid| Reject["Reject plugin\nlog warning"]

    TrustPolicy -->|Yes| Reject
    TrustPolicy -->|No| Caps

    Caps --> ABICheck{"ABI version\ncompatible?"}

    ABICheck -->|Yes| LoadWASM["Load WASM module\nvia Wasmtime"]
    ABICheck -->|No| Reject

    LoadWASM --> ValidateImports["Validate WASM imports\nagainst host ABI allowlist"]
    ValidateImports -->|Pass| Register["Register in\nSdkToolRegistry or\nModelProvider"]
    ValidateImports -->|Fail| Reject

    Register --> Ready["Plugin ready\nfor execution"]
```

## WASM Tool Plugin Execution

```mermaid
sequenceDiagram
    participant B as KelvinBrain
    participant WST as WasmSkillTool
    participant WSH as WasmSkillHost
    participant WM as WASM Module (Guest)
    participant ABI as claw ABI (Host)
    participant Ext as External API

    B->>WST: call(ToolCallInput)
    WST->>WSH: execute(module, args_json)
    WSH->>WSH: Apply SandboxPolicy (fuel, memory)
    WSH->>WM: handle_tool_call(ptr, len)

    alt Plugin needs HTTP
        WM->>ABI: claw::http_call(url, method, headers, body)
        ABI->>ABI: Validate hostname against allowlist
        ABI->>Ext: HTTPS request
        Ext-->>ABI: HTTP response
        ABI-->>WM: response bytes
    end

    alt Plugin needs env var
        WM->>ABI: claw::get_env(key)
        ABI->>ABI: Check env allowlist
        ABI-->>WM: value or empty
    end

    WM->>ABI: claw::log(level, message)
    WM-->>WSH: ToolCallResult JSON (ptr, len)
    WSH-->>WST: ToolCallResult
    WST-->>B: ToolCallResult
```

## WASM Model Plugin Execution

```mermaid
sequenceDiagram
    participant B as KelvinBrain
    participant WMH as WasmModelHost
    participant WM as WASM Module (Guest)
    participant ABI as Model Host ABI
    participant LLM as LLM API

    B->>WMH: infer(ModelInput)
    WMH->>WMH: Apply ModelSandboxPolicy
    WMH->>WM: infer(ptr, len) [ModelInput JSON]

    WM->>ABI: provider_profile_call(endpoint, headers, body)
    ABI->>LLM: HTTPS POST (e.g. api.openai.com/v1/chat/completions)
    LLM-->>ABI: JSON response
    ABI-->>WM: response bytes

    WM->>ABI: log(level, message)
    WM-->>WMH: ModelOutput JSON (ptr, len)
    WMH-->>B: ModelOutput
```

## Memory RPC Flow

```mermaid
sequenceDiagram
    participant B as KelvinBrain
    participant MC as RpcMemoryManager
    participant KMS as AWS KMS
    participant MD as MemoryController
    participant RC as ReplayCache
    participant PR as ProviderRegistry
    participant P as MemoryProvider

    B->>MC: search(query, options)
    MC->>MC: Build DelegationClaims (session_id, ops, exp)

    alt KMS signing
        MC->>KMS: Sign(EdDSA, claims_payload)
        KMS-->>MC: signature
    end

    MC->>MC: Encode JWT (header.claims.signature)
    MC->>MD: gRPC Query(jwt, query, limit)

    MD->>MD: Verify JWT signature + expiry
    MD->>RC: Check replay (jti)
    RC-->>MD: Not replayed

    MD->>PR: route(provider_id, Query)
    PR->>P: query(params)
    P-->>PR: results
    PR-->>MD: results

    MD->>RC: Record jti
    MD-->>MC: QueryResponse(results)
    MC-->>B: Vec of MemorySearchResult
```

## Core Type Relationships

```mermaid
flowchart LR
    subgraph Request["Request Types"]
        ARReq["AgentRunRequest\n- session_id\n- prompt\n- tools_allowed\n- model_hint"]
    end

    subgraph Model["Model Types"]
        MI["ModelInput\n- messages[]\n- tools[]\n- system_prompt"]
        MO["ModelOutput\n- content\n- tool_calls[]\n- usage"]
    end

    subgraph Tool["Tool Types"]
        TD["ToolDefinition\n- name\n- description\n- parameters_schema"]
        TCI["ToolCallInput\n- tool_name\n- arguments"]
        TCR["ToolCallResult\n- output\n- is_error"]
    end

    subgraph Session["Session Types"]
        SM["SessionMessage\n- role\n- content\n- tool_call_id"]
    end

    subgraph Event["Event Types"]
        AE["AgentEvent\n- run_id\n- timestamp\n- data"]
        AED["AgentEventData\n- LifecycleStart\n- AssistantMessage\n- ToolCall\n- ToolResult\n- LifecycleEnd\n- Error"]
    end

    subgraph Result["Result Types"]
        ARRes["AgentRunResult\n- run_id\n- outcome\n- messages"]
        RO["RunOutcome\n- Success\n- Error\n- Cancelled"]
    end

    ARReq -->|"fed into"| MI
    MI -->|"produces"| MO
    MO -->|"may contain"| TCI
    TCI -->|"produces"| TCR
    MO -->|"stored as"| SM
    TCR -->|"stored as"| SM
    AE --> AED
    ARRes --> RO
```
