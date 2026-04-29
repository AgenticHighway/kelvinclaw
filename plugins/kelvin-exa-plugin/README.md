# Kelvin Exa Search

A Kelvin tool plugin that calls the [Exa](https://exa.ai) `/search` API to
answer web search queries, returning titles, URLs, and highlight snippets.
Built on the `wasm_tool_v1` runtime (v2 shared-memory ABI).

## Configuration

| Variable      | Required | Description                                                                                                  |
|---------------|----------|--------------------------------------------------------------------------------------------------------------|
| `EXA_API_KEY` | Yes      | API key from <https://dashboard.exa.ai>. Declared in `capability_scopes.env_allow` so the host can pass it. |

## Tool Input

`tool_name`: `kelvin_exa_search`

| Field                  | Type             | Default | Description                                                                                              |
|------------------------|------------------|---------|----------------------------------------------------------------------------------------------------------|
| `query`                | string           | —       | The search query (required).                                                                             |
| `num_results`          | integer (1-25)   | 5       | Number of results to return.                                                                             |
| `type`                 | string           | `auto`  | One of `auto`, `neural`, or `fast`.                                                                       |
| `category`             | string           | —       | One of `company`, `research paper`, `news`, `personal site`, `financial report`, or `people`.            |
| `include_domains`      | array[string]    | —       | Restrict results to these domains.                                                                       |
| `exclude_domains`      | array[string]    | —       | Drop results from these domains.                                                                         |
| `start_published_date` | string (ISO 8601)| —       | Only return results published on or after this date.                                                     |
| `end_published_date`   | string (ISO 8601)| —       | Only return results published on or before this date.                                                    |
| `livecrawl`            | string           | —       | One of `never`, `fallback`, `preferred`, or `always`.                                                    |
| `summary`              | boolean          | false   | If true, request an LLM-generated summary alongside each result.                                          |

The plugin always asks Exa for `highlights` and a short `text` snippet
(up to 500 characters) so that returned results are immediately useful to
the model.

## Output

Each result is rendered as:

```
1. <title>
   <url>
   <highlight or summary or text snippet>
```

When highlights, summary, and text are all empty for a given result, the
plugin emits the title and URL only.

## Quick Start

```bash
make build        # compile WASM and patch plugin.json SHA-256
make test         # validate manifest structure
make pack         # create dist/kelvin.exa-0.1.0.tar.gz
make install      # install into the local Kelvin plugin home
```

Then set `EXA_API_KEY` in `~/.kelvinclaw/.env`:

```
EXA_API_KEY=your-exa-api-key-here
```

## File Layout

| File          | Purpose                                                          |
|---------------|------------------------------------------------------------------|
| `plugin.json` | Manifest: identity, tool_name, tool_input_schema, capabilities  |
| `src/lib.rs`  | Rust WASM guest: exports alloc/dealloc/handle_tool_call/run     |
| `Cargo.toml`  | Rust crate config (cdylib, no_std, wasm32 target, size opts)    |
| `build.sh`    | Compile, copy .wasm to payload/, update SHA-256 in plugin.json  |
| `Makefile`    | Convenience targets: build, test, pack, install, smoke, clean   |
| `payload/`    | Directory containing the compiled .wasm entrypoint              |

## Sandbox

| Limit                    | Value                |
|--------------------------|----------------------|
| `network_allow_hosts`    | `api.exa.ai`         |
| `env_allow`              | `EXA_API_KEY`        |
| `timeout_ms`             | 30000                |
| `max_calls_per_minute`   | 30                   |

See [`docs/plugins/tool-plugin-abi.md`](../../docs/plugins/tool-plugin-abi.md)
for the underlying tool-plugin ABI.
