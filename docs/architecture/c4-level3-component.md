# C4 Level 3 — Component Diagrams

> What are the key components inside each container?

## kelvin-gateway Components

```mermaid
flowchart TD
    subgraph Gateway["kelvin-gateway"]
        WS["WebSocket Server\n(:34617)\nJSON envelope protocol"]
        Ingress["HTTP Ingress\n(:34618)\nWebhook receiver"]
        ChannelEngine["ChannelEngine\nRoutes inbound messages\nto agent sessions\nTelegram sender data pipe"]
        Scheduler["RuntimeScheduler\nCron + interval triggers\nfor scheduled agent runs"]
        IdempCache["IdempotencyCache\nDeduplicates\nrepeated requests"]
        OpConsole["Operator Console\noperator.* methods\nfor admin queries"]
        SecurityCfg["GatewaySecurityConfig\nTLS, auth, rate limits,\nIP allowlists"]

        WS --> ChannelEngine
        Ingress --> ChannelEngine
        ChannelEngine --> IdempCache
        ChannelEngine -->|"submit_run()"| SDK_Ref["kelvin-sdk"]
        Scheduler -->|"submit_run()"| SDK_Ref
        WS --> OpConsole
    end

    Telegram["Telegram"] -->|POST| Ingress
    Slack["Slack"] -->|POST| Ingress
    Discord["Discord"] -->|POST| Ingress
    WhatsApp["WhatsApp"] -->|POST| Ingress
    TUI["kelvin-tui"] -->|WebSocket| WS
    ExtClient["External WS Client"] -->|WebSocket| WS
```

## kelvin-brain Components

```mermaid
flowchart TD
    subgraph Brain["kelvin-brain"]
        KBrain["KelvinBrain\nAgent loop orchestrator\nvalidate → recall → infer → tool → persist"]
        PluginLoader["InstalledPluginLoader\nScans plugin home dir\nverifies signatures\nvalidates manifests"]
        ToolLoop["ToolLoopDetector\nPrevents infinite\ntool call cycles"]
        WasmSkill["WasmSkillTool\nBridge: Tool trait →\nWASM skill execution"]
        WasmPlugin["WasmSkillPlugin\nBridge: Plugin meta →\nWASM module host"]
        EchoProv["EchoModelProvider\nBuilt-in echo provider\nfor testing"]
        TrustPolicy["PublisherTrustPolicy\nEd25519 signature\nverification"]

        KBrain --> PluginLoader
        KBrain --> ToolLoop
        KBrain --> WasmSkill
        PluginLoader --> TrustPolicy
        PluginLoader --> WasmPlugin
        WasmSkill -->|"delegates to"| WasmHost["kelvin-wasm"]
        WasmPlugin -->|"delegates to"| WasmHost
    end

    Core["kelvin-core traits"] -.->|"Brain, ModelProvider,\nTool, SessionStore,\nEventSink"| KBrain
    Memory["MemorySearchManager"] -.->|"memory recall"| KBrain
```

## kelvin-wasm Components

```mermaid
flowchart TD
    subgraph WASM["kelvin-wasm"]
        SkillHost["WasmSkillHost\nExecutes tool WASM modules\nExport: handle_tool_call()"]
        ModelHost["WasmModelHost\nExecutes model WASM modules\nExport: infer()"]
        ChannelHost["WasmChannelHost\nExecutes channel policy\nWASM modules"]
        SandboxPolicy["SandboxPolicy\nFuel budget, memory limits,\npresets: locked_down /\ndev_local / hardware_control"]
        ABI["claw ABI Layer\nHost functions: http_call,\nget_env, log, clock_now_ms"]
        ImportValidator["Import Validator\nValidates WASM module\nimports against allowlist"]

        SkillHost --> SandboxPolicy
        SkillHost --> ABI
        SkillHost --> ImportValidator
        ModelHost --> SandboxPolicy
        ModelHost --> ABI
        ChannelHost --> SandboxPolicy
        ChannelHost --> ABI
    end

    ABI -->|"host-mediated HTTPS"| ExtAPI["External APIs\n(LLM / Search / etc.)"]
```

## kelvin-sdk Components

```mermaid
flowchart TD
    subgraph SDK["kelvin-sdk"]
        SdkRuntime["KelvinSdkRuntime\nTop-level composition entry\nUsed by gateway + host"]
        SdkConfig["KelvinSdkConfig\nModel selection, memory mode,\nplugin home, session config"]
        ModelSelection["KelvinSdkModelSelection\nEcho / InstalledPlugin /\nInstalledPluginFailover"]
        MemMode["KelvinCliMemoryMode\nRPC / Legacy / Off"]
        Preflight["Plugin Preflight\nVerify plugin health\nbefore run"]
        RunSummary["KelvinRunSummary\nToken counts, latency,\ntool call stats"]
        ToolPack["ToolPack\nBuilt-in safe tools\nwith policy controls"]

        SdkConfig --> ModelSelection
        SdkConfig --> MemMode
        SdkRuntime --> SdkConfig
        SdkRuntime --> Preflight
        SdkRuntime --> ToolPack
        SdkRuntime -->|"orchestrates"| BrainRef["kelvin-brain"]
        SdkRuntime -->|"wires"| MemClientRef["kelvin-memory-client"]
        SdkRuntime --> RunSummary
    end

    GW["kelvin-gateway"] -->|uses| SdkRuntime
    Host["kelvin-host"] -->|uses| SdkRuntime
```

## ToolPack Built-in Tools

```mermaid
flowchart TD
    subgraph ToolPack["ToolPack (kelvin-sdk)"]
        Policy["ToolPackPolicy\nLoaded from env vars\nControls tool availability"]
        FsRead["SafeFsReadTool\nPath-traversal safe\nfile reading"]
        FsWrite["SafeFsWriteTool\nPolicy-gated\nfilesystem writing"]
        WebFetch["SafeWebFetchTool\nHost-allowlisted\nHTTP fetching"]
        SchedTool["SchedulerTool\nCron task management\nvia SDK scheduler"]
        SessionTool["SessionToolsTool\nSession clear /\nhistory management"]

        Policy -->|"gates"| FsRead
        Policy -->|"gates"| FsWrite
        Policy -->|"gates"| WebFetch
        Policy -->|"gates"| SchedTool
        Policy -->|"gates"| SessionTool
    end

    SdkRuntime["KelvinSdkRuntime"] -->|"registers"| ToolPack
    ToolPack -->|"registered into"| ToolReg["SdkToolRegistry"]
```

## Memory Subsystem Components

```mermaid
flowchart TD
    subgraph MemoryPlane["Memory Data Plane"]
        MemClient["RpcMemoryManager\n(kelvin-memory-client)\nImplements MemorySearchManager\nMints JWT delegation tokens"]
        MemAPI["kelvin-memory-api\nProtobuf schema\nv1alpha1.MemoryService\nJWT delegation claims"]
        MemCtrl["MemoryController\n(kelvin-memory-controller)\nValidates JWTs\nReplay protection\nRoutes to providers"]
        ProvReg["ProviderRegistry\nRoutes memory ops\nto provider backends"]
        InMemProv["InMemoryProvider\nVolatile memory\nfor development"]
        ReplayCache["ReplayCache\nPrevents duplicate\noperation replay"]
        ModuleSDK["kelvin-memory-module-sdk\nABI constants for\nWASM memory modules"]
    end

    Brain["kelvin-brain"] -->|"search(query)"| MemClient
    MemClient -->|"gRPC + JWT"| MemCtrl
    MemCtrl --> ReplayCache
    MemCtrl --> ProvReg
    ProvReg --> InMemProv
    ProvReg -->|"WASM modules"| ModuleSDK
    MemClient -->|"sign JWT"| KMS["AWS KMS"]
    MemAPI -.->|"defines contract"| MemClient
    MemAPI -.->|"defines contract"| MemCtrl

    subgraph Legacy["Legacy Memory (deprecated)"]
        Markdown["MarkdownMemoryManager\nWorkspace MEMORY.md files"]
        InMemVec["InMemoryVectorMemoryManager\nVolatile token-overlap index"]
        Fallback["FallbackMemoryManager\nPrimary → fallback wrapper"]
    end
```

## kelvin-core Components

```mermaid
flowchart LR
    subgraph Core["kelvin-core — Trait Contracts"]
        Brain["trait Brain\nrun(request) → result"]
        ModelProvider["trait ModelProvider\ninfer(input) → output"]
        MemSearch["trait MemorySearchManager\nsearch(query) → results"]
        Tool["trait Tool\ncall(input) → result"]
        ToolReg["trait ToolRegistry\nregister / lookup tools"]
        SessionStore["trait SessionStore\nupsert / get / append"]
        EventSink["trait EventSink\nemit agent events"]
        PluginFactory["trait PluginFactory\nbuild plugins from config"]
        PluginRegistry["trait PluginRegistry\nlist / resolve plugins"]
        RunRegistry["trait RunRegistry\nsubmit / track / outcome"]
        CommandProv["trait CommandProvider\nregister CLI commands"]
        CoreRuntime["struct CoreRuntime\nComposite runtime\nwires all traits"]
    end

    CoreRuntime --> Brain
    CoreRuntime --> ModelProvider
    CoreRuntime --> MemSearch
    CoreRuntime --> Tool
    CoreRuntime --> ToolReg
    CoreRuntime --> SessionStore
    CoreRuntime --> EventSink
    CoreRuntime --> RunRegistry
    CoreRuntime --> CommandProv
```
