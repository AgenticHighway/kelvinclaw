# KelvinClaw Documentation

Welcome to KelvinClaw. This documentation is organized by audience and topic.

## 👤 End Users - Getting Started

**New to KelvinClaw?** Start here:

- [Getting Started Guide](getting-started/GETTING_STARTED.md) — Installation, three onboarding tracks, and verification

## 🔧 Contributors & Developers

### Understanding the System

- [Architecture Overview](architecture/architecture.md) — Core design and execution flow
- [Core Admission Policy](architecture/core-admission-policy.md) — What belongs in core vs. extensions
- [SDK Principles](architecture/sdk-principles.md) — Plugin and extension contracts
- [Root vs. SDK Trust Model](architecture/root-vs-sdk.md) — Two extension lanes explained
- [Compatibility Contracts](architecture/compatibility-contracts.md) — Public integration stability guarantees
- [Design Tradeoffs](architecture/agents-tradeoffs.md) — Intentional design decisions
- [Gap Analysis](architecture/kelvin-gap-analysis.md) — Feature completeness tracking

### Building Plugins

- [Plugin Author Kit](plugins/plugin-author-kit.md) — Plugin development workflow
- [Building Model Plugins](plugins/build-a-model-plugin.md) — Custom model provider walkthrough
- [Building Tool Plugins](plugins/build-a-tool-plugin.md) — Custom tool plugin walkthrough
- [Plugin Installation Flow](plugins/plugin-install-flow.md) — Package format and installation
- [Plugin Trust & Quality](plugins/plugin-trust-operations.md) — Signing, verification, and tiers
- [Model Plugin ABI](plugins/model-plugin-abi.md) — `wasm_model_v1` technical spec
- [Tool Plugin ABI](plugins/tool-plugin-abi.md) — `wasm_tool_v1` technical spec
- [Tool Pack Reference](plugins/toolpack-sdk-plugins.md) — Built-in SDK tools

**Provider-Specific Guides:**
- [OpenAI Plugin](plugins/openai-plugin-install-and-run.md)
- [Anthropic Plugin](plugins/anthropic-plugin-install-and-run.md)
- [OpenRouter Plugin](plugins/openrouter-plugin-install-and-run.md)

### Memory System

- [Control/Data Plane Split](memory/memory-control-data-plane.md) — Architecture overview
- [Memory Module SDK](memory/memory-module-sdk.md) — WASM memory extension ABI
- [Memory RPC Contract](memory/memory-rpc-contract.md) — gRPC protocol details
- [Deployment Profiles](memory/memory-controller-deployment-profiles.md) — Configuration and scaling

### Gateway & Integrations

- [Gateway Protocol](gateway/gateway-protocol.md) — WebSocket control plane, methods, and channel routing
- [Channel Plugin ABI](gateway/channel-plugin-abi.md) — WASM ingress policy ABI
- [Terminal UI Integration](gateway/terminal-ui.md) — Local TUI interface

### Security & Compliance

- [Test Matrix](security/sdk-test-matrix.md) — Test coverage overview
- [OWASP Top 10 AI Coverage](security/sdk-owasp-top10-ai-2025.md) — Security test mapping
- [NIST AI RMF Coverage](security/sdk-nist-ai-rmf-1-0.md) — Compliance test mapping

## 📚 Reference & Operations

### Runbooks

- [Memory Module Denial/Timeout Storms](runbooks/memory-module-denial-timeout-storms.md)
- [Publisher Trust Policy](runbooks/module-publisher-trust-policy.md)
- [JWT Key Rotation](runbooks/memory-jwt-key-rotation.md)
- [Gateway Service Management](runbooks/kelvin-gateway-service-management.md)

### Other Resources

- [Kelvin Core SDK](architecture/kelvin-core-sdk.md) — SDK versioning and plugin manifest schema
- [Trusted Executive + WASM](architecture/trusted-executive-wasm.md) — WASM skill sandboxing
- [Plugin Index Schema](plugins/plugin-index-schema.md) — Remote registry format

## 📖 How to Use This Guide

- **I want to use Kelvin** → [Getting Started Guide](getting-started/GETTING_STARTED.md)
- **I'm writing a plugin** → [Plugin Author Kit](plugins/plugin-author-kit.md)
- **I'm understanding the design** → [Architecture Overview](architecture/architecture.md)
- **I'm deploying/operating Kelvin** → [Runbooks](runbooks/)
- **I need a technical spec** → Browse [gateway/](gateway/), [memory/](memory/), [security/](security/)

---

**Note:** Some documentation may be outdated as development is active. For the most current implementation details, consult the codebase.
