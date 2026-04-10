#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEFAULT_CORE_VERSIONS="0.1.0"
DEFAULT_CORE_API_VERSION="1.0.0"

require_cmd() {
  local name="$1"
  if ! command -v "${name}" >/dev/null 2>&1; then
    echo "Missing required command: ${name}" >&2
    exit 1
  fi
}

create_tar_gz() {
  local output_path="$1"
  local base_dir="$2"
  shift 2
  local stage_dir=""
  local rel_path=""
  local src_path=""

  local -a tar_args=(--format ustar -czf "${output_path}")
  if tar --help 2>/dev/null | grep -q -- '--sort='; then
    tar_args=(--sort=name --mtime='UTC 1970-01-01' --owner=0 --group=0 --numeric-owner "${tar_args[@]}")
  fi
  if tar --help 2>/dev/null | grep -q -- '--no-xattrs'; then
    tar_args=(--no-xattrs "${tar_args[@]}")
  fi
  if tar --help 2>/dev/null | grep -q -- '--no-acls'; then
    tar_args=(--no-acls "${tar_args[@]}")
  fi
  if tar --help 2>/dev/null | grep -q -- '--no-selinux'; then
    tar_args=(--no-selinux "${tar_args[@]}")
  fi

  stage_dir="$(mktemp -d)"
  for rel_path in "$@"; do
    src_path="${base_dir}/${rel_path}"
    mkdir -p "${stage_dir}/$(dirname "${rel_path}")"
    if [[ -d "${src_path}" ]]; then
      cp -R "${src_path}" "${stage_dir}/${rel_path}"
    else
      cp -p "${src_path}" "${stage_dir}/${rel_path}"
    fi
  done
  if command -v xattr >/dev/null 2>&1; then
    xattr -rc "${stage_dir}" >/dev/null 2>&1 || true
  fi

  COPYFILE_DISABLE=1 COPY_EXTENDED_ATTRIBUTES_DISABLE=1 tar "${tar_args[@]}" -C "${stage_dir}" "$@"
  rm -rf "${stage_dir}"
}

sha256_file() {
  local file="$1"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "${file}" | awk '{print $1}'
    return
  fi
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "${file}" | awk '{print $1}'
    return
  fi
  echo "Missing required command: shasum or sha256sum" >&2
  exit 1
}

semver_valid() {
  [[ "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.-]+)?$ ]]
}

semver_ge() {
  local left="$1"
  local right="$2"
  [[ "$(printf '%s\n%s\n' "${left}" "${right}" | sort -V | tail -n1)" == "${left}" ]]
}

semver_le() {
  local left="$1"
  local right="$2"
  [[ "$(printf '%s\n%s\n' "${left}" "${right}" | sort -V | head -n1)" == "${left}" ]]
}

quality_tier_valid() {
  case "$1" in
    unsigned_local|signed_community|signed_trusted) return 0 ;;
    *) return 1 ;;
  esac
}

builtin_provider_profile_json() {
  case "$1" in
    openai.responses)
      printf '%s\n' '{
  "id": "openai.responses",
  "provider_name": "openai",
  "protocol_family": "openai_responses",
  "api_key_env": "OPENAI_API_KEY",
  "base_url_env": "OPENAI_BASE_URL",
  "default_base_url": "https://api.openai.com",
  "endpoint_path": "v1/responses",
  "auth_header": "authorization",
  "auth_scheme": "bearer",
  "static_headers": [],
  "default_allow_hosts": ["api.openai.com"]
}'
      ;;
    anthropic.messages)
      printf '%s\n' '{
  "id": "anthropic.messages",
  "provider_name": "anthropic",
  "protocol_family": "anthropic_messages",
  "api_key_env": "ANTHROPIC_API_KEY",
  "base_url_env": "ANTHROPIC_BASE_URL",
  "default_base_url": "https://api.anthropic.com",
  "endpoint_path": "v1/messages",
  "auth_header": "x-api-key",
  "auth_scheme": "raw",
  "static_headers": [
    {
      "name": "anthropic-version",
      "value": "2023-06-01"
    }
  ],
  "default_allow_hosts": ["api.anthropic.com"]
}'
      ;;
    *)
      return 1
      ;;
  esac
}

provider_profile_default_provider_name() {
  case "$1" in
    openai.responses) printf '%s' "openai" ;;
    anthropic.messages) printf '%s' "anthropic" ;;
    *) return 1 ;;
  esac
}

protocol_family_default_model_name() {
  local protocol_family="$1"
  local provider_name="${2:-}"
  case "${protocol_family}" in
    openai_responses) printf '%s' "gpt-4.1-mini" ;;
    anthropic_messages) printf '%s' "claude-haiku-4-5-20251001" ;;
    openai_chat_completions)
      if [[ "${provider_name}" == "openrouter" ]]; then
        printf '%s' "openai/gpt-4.1-mini"
      else
        printf '%s' "default"
      fi
      ;;
    *)
      printf '%s' "default"
      ;;
  esac
}

scaffold_tool_plugin_project() {
  local output_dir="$1"
  local plugin_id="$2"
  local display_name="$3"
  local plugin_version="$4"
  local entrypoint_rel="$5"
  local crate_package_name="$6"
  local crate_lib_name="$7"

  mkdir -p "${output_dir}/src" "${output_dir}/payload"

  cat > "${output_dir}/Cargo.toml" <<EOF
# Kelvin tool plugin crate.
# Compiled to wasm32-unknown-unknown and loaded by the Kelvin runtime as a
# wasm_tool_v1 plugin. Distributed as a .wasm binary, not published to crates.io.

[package]
name = "${crate_package_name}"
# edition = "2021" is the minimum for stable wasm32 support.
version = "${plugin_version}"
edition = "2021"
publish = false

[lib]
# cdylib produces a .wasm with C ABI exports (alloc, dealloc, handle_tool_call).
# Do NOT add rlib here; Kelvin loads only the .wasm cdylib.
name = "${crate_lib_name}"
crate-type = ["cdylib"]

# Size optimisations — tool plugins are typically very small (<10 KiB).
[profile.release]
opt-level = "s"   # optimise for size
lto = true         # link-time optimisation
strip = true       # strip debug symbols

# Empty [workspace] prevents Cargo from walking up to a parent workspace.
[workspace]
EOF

  cat > "${output_dir}/src/lib.rs" <<'EOF'
//! Kelvin tool plugin guest — wasm_tool_v1 ABI (v2 shared-memory path).
//!
//! # Required exports (all must be present or the plugin will fail to load)
//!
//! | Export            | Signature            | Purpose                                     |
//! |-------------------|----------------------|---------------------------------------------|
//! | `memory`          | Memory               | Linear memory shared between host and guest |
//! | `alloc`           | `(i32) -> i32`       | Bump-allocate N bytes; returns pointer or 0 |
//! | `dealloc`         | `(i32, i32) -> ()`   | Free allocation (no-op in arena allocators) |
//! | `handle_tool_call`| `(i32, i32) -> i64`  | Main entry point (v2 ABI — see below)       |
//! | `run`             | `() -> i32`          | v1 compatibility stub; return 0             |
//!
//! # handle_tool_call return convention
//!
//! The return value is a packed i64:
//!   upper 32 bits = pointer to output JSON in guest memory
//!   lower 32 bits = byte length of the output JSON
//! Return 0 to signal an error.
//!
//! # Host imports (claw module)
//!
//! All imports are under the `claw` module. `log` is always available;
//! `network_send` and `fs_read` require the corresponding capability scopes
//! declared in plugin.json and approved by the host security policy.

#![no_std]

#[link(wasm_import_module = "claw")]
extern "C" {
    /// Log a UTF-8 message to the Kelvin host log.
    ///
    /// level: 0=trace, 1=debug, 2=info, 3=warn, 4=error
    /// msg_ptr / msg_len: byte slice in guest memory.
    /// Returns 0 (reserved).
    #[allow(dead_code)]
    fn log(level: i32, msg_ptr: i32, msg_len: i32) -> i32;

    /// Send a packet over the network.
    /// Requires `network_egress` capability and host in `network_allow_hosts`.
    #[allow(dead_code)]
    fn network_send(packet: i32) -> i32;

    /// Read from a filesystem path.
    /// Requires `fs_read` capability and path in `fs_read_paths`.
    #[allow(dead_code)]
    fn fs_read(handle: i32) -> i32;
}

// ---------------------------------------------------------------------------
// Arena allocator
//
// No libc is available in a no_std WASM guest. We manage a 1 MiB static
// heap with a bump pointer. The 8-byte alignment satisfies all scalar types.
// `dealloc` is intentionally a no-op: the host creates a fresh WASM instance
// per call, so all memory is reclaimed when the instance exits.
// ---------------------------------------------------------------------------

const HEAP_SIZE: usize = 1024 * 1024; // 1 MiB
static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
static mut NEXT_OFFSET: usize = 0;

#[no_mangle]
pub extern "C" fn alloc(len: i32) -> i32 {
    if len <= 0 {
        return 0;
    }
    let len = len as usize;
    let align = 8usize;
    unsafe {
        let start = (NEXT_OFFSET + (align - 1)) & !(align - 1);
        let Some(end) = start.checked_add(len) else {
            return 0;
        };
        if end > HEAP_SIZE {
            return 0;
        }
        NEXT_OFFSET = end;
        core::ptr::addr_of_mut!(HEAP).cast::<u8>().add(start) as usize as i32
    }
}

#[no_mangle]
pub extern "C" fn dealloc(_ptr: i32, _len: i32) {}

// ---------------------------------------------------------------------------
// handle_tool_call — main entry point (v2 ABI)
//
// The host writes the tool arguments JSON directly into guest memory at (ptr, len).
//
// Input JSON — the tool arguments object (matches tool_input_schema in plugin.json):
//   { "my_arg": "value", ... }
//
// NOTE: the input is the raw arguments object, NOT a wrapped ToolCallInput struct.
// Do NOT look for "run_id", "session_id", "workspace_dir", or "arguments" fields —
// those are not present. Parse your tool's own fields directly from the input bytes.
//
// Must return a ToolCallResult JSON:
//   summary       string       — short description of what was done (shown in logs)
//   output        string|null  — the tool's primary output (shown to the model)
//   visible_text  string|null  — human-readable output (shown in UI if different from output)
//   is_error      bool         — true if the tool call failed
//
// The return value is a packed i64 (ptr << 32 | len) pointing at the result JSON.
// Return 0 to signal a hard error.
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn handle_tool_call(ptr: i32, len: i32) -> i64 {
    if len <= 0 {
        return 0;
    }
    let _input = unsafe { core::slice::from_raw_parts(ptr as *const u8, len as usize) };

    // TODO: parse _input JSON, execute tool logic, build result JSON.
    //
    // Minimal valid ToolCallResult:
    let result = b"{\"summary\":\"ok\",\"output\":null,\"visible_text\":null,\"is_error\":false}";

    let out_ptr = alloc(result.len() as i32);
    if out_ptr == 0 {
        return 0;
    }
    unsafe {
        core::ptr::copy_nonoverlapping(result.as_ptr(), out_ptr as *mut u8, result.len());
    }
    ((out_ptr as i64) << 32) | (result.len() as i64)
}

// v1 ABI backward-compatibility stub — the host calls this on older runtimes
// that do not support handle_tool_call. Return 0 (success, no output).
#[no_mangle]
pub extern "C" fn run() -> i32 {
    0
}

// ---------------------------------------------------------------------------
// Example: using log() from within handle_tool_call
//
// fn log_str(level: i32, msg: &[u8]) {
//     let ptr = alloc(msg.len() as i32);
//     if ptr == 0 { return; }
//     unsafe {
//         core::ptr::copy_nonoverlapping(msg.as_ptr(), ptr as *mut u8, msg.len());
//         log(level, ptr, msg.len() as i32);
//     }
// }
//
// Then inside handle_tool_call:
//   log_str(2, b"tool called");
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Example: minimal JSON field extractor (no_std, no allocator)
//
// fn extract_str_field<'a>(json: &'a [u8], field: &[u8]) -> Option<&'a [u8]> {
//     let mut needle = [0u8; 64];
//     let mut ni = 0;
//     needle[ni] = b'"'; ni += 1;
//     for &b in field { needle[ni] = b; ni += 1; }
//     needle[ni] = b'"'; ni += 1;
//     needle[ni] = b':'; ni += 1;
//     needle[ni] = b'"'; ni += 1;
//     let needle = &needle[..ni];
//     let pos = json.windows(needle.len()).position(|w| w == needle)?;
//     let start = pos + needle.len();
//     let mut i = start;
//     while i < json.len() {
//         if json[i] == b'\\' { i += 2; continue; }
//         if json[i] == b'"' { return Some(&json[start..i]); }
//         i += 1;
//     }
//     None
// }
// ---------------------------------------------------------------------------

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
EOF

  cat > "${output_dir}/build.sh" <<EOF
#!/usr/bin/env bash
# build.sh — Compile the Kelvin tool plugin WASM binary.
#
# Steps:
#   1. Ensure the wasm32-unknown-unknown rustup target is installed
#   2. cargo build --release for the wasm32 target (produces a cdylib .wasm)
#   3. Copy the .wasm into payload/ (where plugin.json's "entrypoint" points)
#   4. Compute SHA-256 of the .wasm and patch plugin.json's entrypoint_sha256
#
# The SHA-256 is verified at install-time and load-time by the Kelvin runtime
# to ensure the binary on disk matches the manifest declaration.
set -euo pipefail

ROOT_DIR="\$(cd "\$(dirname "\${BASH_SOURCE[0]}")" && pwd)"
PLUGIN_JSON="\${ROOT_DIR}/plugin.json"
PAYLOAD_DIR="\${ROOT_DIR}/payload"
ENTRYPOINT_REL="\$(jq -er '.entrypoint' "\${PLUGIN_JSON}")"
ENTRYPOINT_ABS="\${PAYLOAD_DIR}/\${ENTRYPOINT_REL}"
TARGET_ROOT="\${CARGO_TARGET_DIR:-\${ROOT_DIR}/target}"
TARGET_DIR="\${TARGET_ROOT}/wasm32-unknown-unknown/release"
WASM_SOURCE="\${TARGET_DIR}/${crate_lib_name}.wasm"

require_cmd() {
  local name="\$1"
  if ! command -v "\${name}" >/dev/null 2>&1; then
    echo "Missing required command: \${name}" >&2
    exit 1
  fi
}

sha256_file() {
  local file="\$1"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "\${file}" | awk '{print \$1}'
    return
  fi
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "\${file}" | awk '{print \$1}'
    return
  fi
  echo "Missing required command: shasum or sha256sum" >&2
  exit 1
}

require_cmd cargo
require_cmd jq
require_cmd rustup

# Step 1: ensure the cross-compilation target is available.
rustup target add wasm32-unknown-unknown >/dev/null

# Step 2: compile the Rust guest to a WASM cdylib.
cargo build --release --target wasm32-unknown-unknown

# Step 3: copy the compiled binary into payload/ where the manifest expects it.
mkdir -p "\$(dirname "\${ENTRYPOINT_ABS}")"
cp "\${WASM_SOURCE}" "\${ENTRYPOINT_ABS}"

# Step 4: compute and record the SHA-256 so the runtime can verify integrity.
ENTRYPOINT_SHA="\$(sha256_file "\${ENTRYPOINT_ABS}")"
jq --arg sha "\${ENTRYPOINT_SHA}" '.entrypoint_sha256 = \$sha' "\${PLUGIN_JSON}" > "\${PLUGIN_JSON}.tmp"
mv "\${PLUGIN_JSON}.tmp" "\${PLUGIN_JSON}"

echo "[kelvin-plugin] built ${plugin_id} -> \${ENTRYPOINT_ABS}"
echo "[kelvin-plugin] entrypoint sha256: \${ENTRYPOINT_SHA}"
EOF
  chmod +x "${output_dir}/build.sh"

  cat > "${output_dir}/Makefile" <<EOF
# Makefile for ${display_name}
# Convenience wrapper around the common kelvin-plugin.sh development commands.
# Run 'make help' to list available targets.

PLUGIN_DIR := \$(dir \$(abspath \$(lastword \$(MAKEFILE_LIST))))
MANIFEST   := \$(PLUGIN_DIR)plugin.json
SCRIPTS    := \$(PLUGIN_DIR)../scripts

ID      := \$(shell jq -er '.id'      \$(MANIFEST) 2>/dev/null || echo unknown)
VERSION := \$(shell jq -er '.version' \$(MANIFEST) 2>/dev/null || echo 0.0.0)
PACKAGE := \$(PLUGIN_DIR)dist/\$(ID)-\$(VERSION).tar.gz

.PHONY: build test pack install smoke clean help

build:          ## Compile WASM and update entrypoint_sha256 in plugin.json
	@bash \$(PLUGIN_DIR)build.sh

test: build     ## Validate plugin manifest structure
	@\$(SCRIPTS)/kelvin-plugin.sh test --manifest \$(MANIFEST)

pack: build     ## Create distributable .tar.gz package in dist/
	@\$(SCRIPTS)/kelvin-plugin.sh pack --manifest \$(MANIFEST)

install: pack   ## Install plugin into the local Kelvin plugin home
	@\$(SCRIPTS)/kelvin-plugin.sh install --package \$(PACKAGE)

smoke: build    ## Run a local smoke test
	@\$(SCRIPTS)/kelvin-plugin.sh smoke --manifest \$(MANIFEST)

clean:          ## Remove build artifacts (target/, dist/, payload/*.wasm)
	@rm -rf \$(PLUGIN_DIR)target \$(PLUGIN_DIR)dist \$(PLUGIN_DIR)payload/*.wasm

help:           ## Show this help message
	@grep -E '^[a-zA-Z_-]+:.*?## ' \$(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  %-10s %s\\n", \$\$1, \$\$2}'
EOF

  cat > "${output_dir}/payload/README.md" <<EOF
This directory holds the compiled WASM entrypoint for the plugin.

Run \`./build.sh\` (or \`make build\`) to compile \`src/lib.rs\` and place the
resulting \`.wasm\` file here as \`${entrypoint_rel}\`.

The filename must match the \`entrypoint\` field in \`plugin.json\`. The Kelvin
runtime verifies the file's SHA-256 against \`entrypoint_sha256\` in the manifest;
\`build.sh\` keeps that field up to date automatically.

\`.wasm\` files are excluded from version control (see \`.gitignore\`).
EOF

  cat > "${output_dir}/.gitignore" <<'EOF'
# Build artifacts — do not commit compiled WASM binaries or distribution archives.
/dist/
/target/
/payload/*.wasm
EOF

  cat > "${output_dir}/README.md" <<EOF
# ${display_name}

A Kelvin tool plugin using the \`wasm_tool_v1\` runtime (v2 shared-memory ABI).
Generated by \`scripts/kelvin-plugin.sh new --runtime wasm_tool_v1\`.

## Architecture

Kelvin tool plugins are sandboxed WASM guests. The host calls \`handle_tool_call\`
with a \`ToolCallInput\` JSON, the guest executes arbitrary logic and returns a
\`ToolCallResult\` JSON. The guest runs in a fresh WASM instance per call with a
fuel budget, memory limits, and a network/filesystem allowlist enforced by the host.

## File Layout

| File               | Purpose                                                          |
|--------------------|------------------------------------------------------------------|
| \`plugin.json\`      | Manifest: identity, tool_name, tool_input_schema, capabilities  |
| \`src/lib.rs\`       | Rust WASM guest: exports alloc/dealloc/handle_tool_call/run     |
| \`Cargo.toml\`       | Rust crate config (cdylib, no_std, wasm32 target, size opts)    |
| \`build.sh\`         | Compile, copy .wasm to payload/, update SHA-256 in plugin.json  |
| \`Makefile\`         | Convenience targets: build, test, pack, install, smoke, clean   |
| \`payload/\`         | Directory containing the compiled .wasm entrypoint              |

## Quick Start

\`\`\`bash
make build        # compile WASM and patch plugin.json SHA-256
make test         # validate manifest structure
make smoke        # run a local smoke test
make pack         # create dist/${plugin_id}-${plugin_version}.tar.gz
make install      # install into the local Kelvin plugin home
\`\`\`

## Implementing Your Tool

Edit \`src/lib.rs\`. The \`handle_tool_call\` function is your entry point.
It receives the tool arguments as JSON and must return a \`ToolCallResult\` JSON.

### Input JSON (host → guest)

The input passed to \`handle_tool_call\` is the **raw arguments object** — the exact
JSON the model supplied, matching your \`tool_input_schema\`. There is no outer wrapper.

\`\`\`
{ "my_arg": "value", ... }
\`\`\`

**Do not** look for \`run_id\`, \`session_id\`, \`workspace_dir\`, or \`arguments\` in the
input — those fields are not present. Parse your own argument fields directly.

### ToolCallResult JSON (guest → host)

\`\`\`
{
  "summary":      string       — short description of what was done (shown in logs)
  "output":       string|null  — primary output returned to the model
  "visible_text": string|null  — human-readable output for the UI (if different)
  "is_error":     bool         — true if the tool call failed
}
\`\`\`

### Defining Your Input Schema

Edit \`tool_input_schema\` in \`plugin.json\` to declare your tool's arguments.
This is a standard JSON Schema object. Example:

\`\`\`json
"tool_input_schema": {
  "type": "object",
  "properties": {
    "query": { "type": "string", "description": "The search query" },
    "limit": { "type": "integer", "description": "Max results", "default": 10 }
  },
  "required": ["query"]
}
\`\`\`

## Available Host Imports (\`claw\` module)

| Import         | Signature                  | Requires capability    |
|----------------|----------------------------|------------------------|
| \`log\`          | \`(i32, i32, i32) -> i32\`  | Always available       |
| \`network_send\` | \`(i32) -> i32\`            | \`network_egress\` cap  |
| \`fs_read\`      | \`(i32) -> i32\`            | \`fs_read\` cap         |

To enable \`network_send\`, add to \`plugin.json\`:
\`\`\`json
"capabilities": ["tool_provider", "network_egress"],
"capability_scopes": {
  "network_allow_hosts": ["api.example.com"]
}
\`\`\`

## Sandbox Limits

| Limit               | Default  |
|---------------------|----------|
| Max module size     | 512 KiB  |
| Max request JSON    | 256 KiB  |
| Max response JSON   | 256 KiB  |
| Fuel budget         | 1 000 000 |
| Timeout             | 2 000 ms |

## Packaging & Distribution

\`\`\`bash
make pack         # creates dist/${plugin_id}-${plugin_version}.tar.gz
make install      # installs from the packaged tarball
\`\`\`

Local development plugins can stay \`unsigned_local\`. Kelvin prints a warning
on install but still loads the plugin from a local plugin home.

To sign for distribution:

\`\`\`bash
scripts/plugin-sign.sh \\\\
  --manifest ./plugin.json \\\\
  --private-key /path/to/ed25519-private.pem \\\\
  --publisher-id your.publisher.id \\\\
  --trust-policy-out ./trusted_publishers.json
\`\`\`
EOF
}

scaffold_model_plugin_project() {
  local output_dir="$1"
  local plugin_id="$2"
  local display_name="$3"
  local plugin_version="$4"
  local entrypoint_rel="$5"
  local crate_package_name="$6"
  local crate_lib_name="$7"

  mkdir -p "${output_dir}/src" "${output_dir}/payload"

  cat > "${output_dir}/Cargo.toml" <<EOF
# Kelvin model plugin crate.
# Compiled to wasm32-unknown-unknown and loaded by the Kelvin runtime as a
# wasm_model_v1 plugin. Distributed as a .wasm binary, not published to crates.io.

[package]
name = "${crate_package_name}"
# edition = "2021" is the minimum for stable wasm32 support.
version = "${plugin_version}"
edition = "2021"
publish = false

[lib]
# cdylib produces a .wasm with C ABI exports (alloc, dealloc, infer).
# Do NOT add rlib here; Kelvin loads only the .wasm cdylib.
name = "${crate_lib_name}"
crate-type = ["cdylib"]

# Uncomment to optimise for binary size (typical plugin .wasm < 2 KiB):
# [profile.release]
# opt-level = "z"     # optimise for size
# lto = true          # link-time optimisation across the single crate
# strip = "symbols"   # strip debug symbols

# Empty [workspace] prevents Cargo from walking up to a parent workspace.
[workspace]
EOF

  cat > "${output_dir}/src/lib.rs" <<'EOF'
//! Kelvin model plugin guest — wasm_model_v1 ABI.
//!
//! # Required exports (all must be present or the plugin will fail to load)
//!
//! | Export    | Signature            | Purpose                                          |
//! |-----------|----------------------|--------------------------------------------------|
//! | `memory`  | Memory               | Linear memory shared between host and guest      |
//! | `alloc`   | `(i32) -> i32`       | Bump-allocate N bytes; returns pointer or 0      |
//! | `dealloc` | `(i32, i32) -> ()`   | Free allocation (no-op in this arena allocator)  |
//! | `infer`   | `(i32, i32) -> i64`  | Main entry point (see below)                     |
//!
//! # infer return convention
//!
//! The return value is a packed i64:
//!   upper 32 bits = pointer into guest memory
//!   lower 32 bits = byte length of the response JSON
//! Return 0 to signal an error.
//!
//! # Host imports (kelvin_model_host_v1)
//!
//! The host provides the following imports. All are optional to call, but
//! `provider_profile_call` is what makes the actual HTTP request for most plugins.

#![no_std]

#[link(wasm_import_module = "kelvin_model_host_v1")]
extern "C" {
    /// Delegate the request to the provider declared in plugin.json's `provider_profile`.
    ///
    /// The host reads the `ModelInput` JSON from guest memory at (req_ptr, req_len),
    /// translates it to the provider's native protocol (Anthropic Messages,
    /// OpenAI Responses, or OpenAI Chat Completions), makes the HTTP call,
    /// and writes a `ModelOutput` JSON response back into guest memory via `alloc`.
    ///
    /// Returns a packed i64 (ptr << 32 | len) pointing at the response, or 0 on error.
    fn provider_profile_call(req_ptr: i32, req_len: i32) -> i64;

    /// Same as provider_profile_call but always uses the built-in OpenAI Responses
    /// profile, ignoring plugin.json. Useful only if you need to hard-code OpenAI
    /// regardless of the manifest's provider_profile.
    #[allow(dead_code)]
    fn openai_responses_call(req_ptr: i32, req_len: i32) -> i64;

    /// Log a UTF-8 message to the Kelvin host log.
    ///
    /// level: 0=trace, 1=debug, 2=info, 3=warn, 4=error
    /// msg_ptr / msg_len: byte slice in guest memory.
    /// Returns 0 (reserved).
    #[allow(dead_code)]
    fn log(level: i32, msg_ptr: i32, msg_len: i32) -> i32;

    /// Returns the current wall-clock time as milliseconds since Unix epoch.
    #[allow(dead_code)]
    fn clock_now_ms() -> i64;
}

// ---------------------------------------------------------------------------
// Arena allocator
//
// No libc is available in a no_std WASM guest. We manage a 1 MiB static
// heap with a bump pointer. The 8-byte alignment satisfies all scalar types.
// `dealloc` is intentionally a no-op: the host creates a fresh WASM instance
// per call, so all memory is reclaimed when the instance exits.
// ---------------------------------------------------------------------------

const HEAP_SIZE: usize = 1024 * 1024; // 1 MiB — sufficient for typical JSON payloads
static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
static mut NEXT_OFFSET: usize = 0;

#[no_mangle]
pub extern "C" fn alloc(len: i32) -> i32 {
    if len <= 0 {
        return 0;
    }

    let len = len as usize;
    let align = 8usize;

    unsafe {
        let start = (NEXT_OFFSET + (align - 1)) & !(align - 1);
        let Some(end) = start.checked_add(len) else {
            return 0;
        };
        if end > HEAP_SIZE {
            return 0;
        }
        NEXT_OFFSET = end;
        core::ptr::addr_of_mut!(HEAP).cast::<u8>().add(start) as usize as i32
    }
}

#[no_mangle]
pub extern "C" fn dealloc(_ptr: i32, _len: i32) {}

// ---------------------------------------------------------------------------
// infer — main entry point
//
// The host serialises a ModelInput into JSON and writes it into guest memory
// at (req_ptr, req_len). This passthrough implementation delegates directly
// to provider_profile_call, which handles protocol translation and the HTTP
// request. The return value is a packed (ptr << 32 | len) pointing at a
// ModelOutput JSON in guest memory.
//
// ModelInput JSON fields:
//   run_id          string   — unique identifier for this inference call
//   session_id      string   — session the call belongs to
//   system_prompt   string   — the system/context prompt
//   user_prompt     string   — the user's message
//   memory_snippets []string — relevant memory snippets injected by the host
//   history         []       — prior session messages
//   tools           []       — tool definitions available to the model
//
// ModelOutput JSON fields (must be returned by a custom infer):
//   assistant_text  string   — the model's text response
//   stop_reason     string?  — why generation stopped (e.g. "end_turn")
//   tool_calls      []       — any tool calls the model wants to make
//   usage           object?  — token usage stats (optional)
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn infer(req_ptr: i32, req_len: i32) -> i64 {
    // SAFETY: The trusted Kelvin host provides this import for approved
    // provider_profile-backed model plugins.
    unsafe { provider_profile_call(req_ptr, req_len) }
}

// ---------------------------------------------------------------------------
// Example: using log() from within infer
//
// Uncomment to emit a debug log on every inference call.
//
// fn log_str(level: i32, msg: &[u8]) {
//     let ptr = alloc(msg.len() as i32);
//     if ptr == 0 { return; }
//     unsafe {
//         core::ptr::copy_nonoverlapping(msg.as_ptr(), ptr as *mut u8, msg.len());
//         log(level, ptr, msg.len() as i32);
//     }
// }
//
// Then inside infer:
//   log_str(2, b"infer called");
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Example: using clock_now_ms()
//
// let _now_ms: i64 = unsafe { clock_now_ms() };
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Example: custom infer (offline / echo style, no HTTP call)
//
// Override infer to build a ModelOutput JSON directly in guest memory,
// without calling any host import. Useful for mocking, testing, or building
// a fully self-contained plugin.
//
// #[no_mangle]
// pub extern "C" fn infer(req_ptr: i32, req_len: i32) -> i64 {
//     let input = unsafe {
//         core::slice::from_raw_parts(req_ptr as *const u8, req_len as usize)
//     };
//     // Extract the user_prompt field from the input JSON.
//     let prompt = extract_str_field(input, b"user_prompt").unwrap_or(b"(no prompt)");
//
//     // Build a minimal ModelOutput JSON.
//     const PRE:  &[u8] = b"{\"assistant_text\":\"";
//     const POST: &[u8] = b"\",\"stop_reason\":\"end_turn\",\"tool_calls\":[],\"usage\":null}";
//     let total = PRE.len() + prompt.len() + POST.len();
//     let ptr = alloc(total as i32) as usize;
//     if ptr == 0 { return 0; }
//     unsafe {
//         let base = ptr as *mut u8;
//         let mut off = 0;
//         core::ptr::copy_nonoverlapping(PRE.as_ptr(),    base.add(off), PRE.len());    off += PRE.len();
//         core::ptr::copy_nonoverlapping(prompt.as_ptr(), base.add(off), prompt.len()); off += prompt.len();
//         core::ptr::copy_nonoverlapping(POST.as_ptr(),   base.add(off), POST.len());
//     }
//     ((ptr as i64) << 32) | (total as i64)
// }
//
// // Minimal JSON field extractor (no_std, no allocator needed).
// fn extract_str_field<'a>(json: &'a [u8], field: &[u8]) -> Option<&'a [u8]> {
//     let mut needle = [0u8; 64];
//     let mut ni = 0;
//     needle[ni] = b'"'; ni += 1;
//     for &b in field { needle[ni] = b; ni += 1; }
//     needle[ni] = b'"'; ni += 1;
//     needle[ni] = b':'; ni += 1;
//     needle[ni] = b'"'; ni += 1;
//     let needle = &needle[..ni];
//     let pos = json.windows(needle.len()).position(|w| w == needle)?;
//     let start = pos + needle.len();
//     let mut i = start;
//     while i < json.len() {
//         if json[i] == b'\\' { i += 2; continue; }
//         if json[i] == b'"' { return Some(&json[start..i]); }
//         i += 1;
//     }
//     None
// }
// ---------------------------------------------------------------------------

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
EOF

  cat > "${output_dir}/build.sh" <<EOF
#!/usr/bin/env bash
# build.sh — Compile the Kelvin model plugin WASM binary.
#
# Steps:
#   1. Ensure the wasm32-unknown-unknown rustup target is installed
#   2. cargo build --release for the wasm32 target (produces a cdylib .wasm)
#   3. Copy the .wasm into payload/ (where plugin.json's "entrypoint" points)
#   4. Compute SHA-256 of the .wasm and patch plugin.json's entrypoint_sha256
#
# The SHA-256 is verified at install-time and load-time by the Kelvin runtime
# to ensure the binary on disk matches the manifest declaration.
set -euo pipefail

ROOT_DIR="\$(cd "\$(dirname "\${BASH_SOURCE[0]}")" && pwd)"
PLUGIN_JSON="\${ROOT_DIR}/plugin.json"
PAYLOAD_DIR="\${ROOT_DIR}/payload"
# Read the entrypoint filename from plugin.json so build.sh stays in sync
# with any manual edits to the manifest.
ENTRYPOINT_REL="\$(jq -er '.entrypoint' "\${PLUGIN_JSON}")"
ENTRYPOINT_ABS="\${PAYLOAD_DIR}/\${ENTRYPOINT_REL}"
TARGET_ROOT="\${CARGO_TARGET_DIR:-\${ROOT_DIR}/target}"
TARGET_DIR="\${TARGET_ROOT}/wasm32-unknown-unknown/release"
WASM_SOURCE="\${TARGET_DIR}/${crate_lib_name}.wasm"

require_cmd() {
  local name="\$1"
  if ! command -v "\${name}" >/dev/null 2>&1; then
    echo "Missing required command: \${name}" >&2
    exit 1
  fi
}

sha256_file() {
  local file="\$1"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "\${file}" | awk '{print \$1}'
    return
  fi
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "\${file}" | awk '{print \$1}'
    return
  fi
  echo "Missing required command: shasum or sha256sum" >&2
  exit 1
}

require_cmd cargo
require_cmd jq
require_cmd rustup

# Step 1: ensure the cross-compilation target is available.
rustup target add wasm32-unknown-unknown >/dev/null

# Step 2: compile the Rust guest to a WASM cdylib.
cargo build --release --target wasm32-unknown-unknown

# Step 3: copy the compiled binary into payload/ where the manifest expects it.
mkdir -p "\$(dirname "\${ENTRYPOINT_ABS}")"
cp "\${WASM_SOURCE}" "\${ENTRYPOINT_ABS}"

# Step 4: compute and record the SHA-256 so the runtime can verify integrity.
ENTRYPOINT_SHA="\$(sha256_file "\${ENTRYPOINT_ABS}")"
jq --arg sha "\${ENTRYPOINT_SHA}" '.entrypoint_sha256 = \$sha' "\${PLUGIN_JSON}" > "\${PLUGIN_JSON}.tmp"
mv "\${PLUGIN_JSON}.tmp" "\${PLUGIN_JSON}"

echo "[kelvin-plugin] built ${plugin_id} -> \${ENTRYPOINT_ABS}"
echo "[kelvin-plugin] entrypoint sha256: \${ENTRYPOINT_SHA}"
EOF
  chmod +x "${output_dir}/build.sh"

  cat > "${output_dir}/Makefile" <<EOF
# Makefile for ${display_name}
# Convenience wrapper around the common kelvin-plugin.sh development commands.
# Run 'make help' to list available targets.

PLUGIN_DIR := \$(dir \$(abspath \$(lastword \$(MAKEFILE_LIST))))
MANIFEST   := \$(PLUGIN_DIR)plugin.json
SCRIPTS    := \$(PLUGIN_DIR)../scripts

ID      := \$(shell jq -er '.id'      \$(MANIFEST) 2>/dev/null || echo unknown)
VERSION := \$(shell jq -er '.version' \$(MANIFEST) 2>/dev/null || echo 0.0.0)
PACKAGE := \$(PLUGIN_DIR)dist/\$(ID)-\$(VERSION).tar.gz

.PHONY: build test pack install smoke clean help

build:          ## Compile WASM and update entrypoint_sha256 in plugin.json
	@bash \$(PLUGIN_DIR)build.sh

test: build     ## Validate plugin manifest structure
	@\$(SCRIPTS)/kelvin-plugin.sh test --manifest \$(MANIFEST)

pack: build     ## Create distributable .tar.gz package in dist/
	@\$(SCRIPTS)/kelvin-plugin.sh pack --manifest \$(MANIFEST)

install: pack   ## Install plugin into the local Kelvin plugin home
	@\$(SCRIPTS)/kelvin-plugin.sh install --package \$(PACKAGE)

smoke: build    ## Run a live inference smoke test (API key env var must be set)
	@\$(SCRIPTS)/kelvin-plugin.sh smoke --manifest \$(MANIFEST)

clean:          ## Remove build artifacts (target/, dist/, payload/*.wasm)
	@rm -rf \$(PLUGIN_DIR)target \$(PLUGIN_DIR)dist \$(PLUGIN_DIR)payload/*.wasm

help:           ## Show this help message
	@grep -E '^[a-zA-Z_-]+:.*?## ' \$(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  %-10s %s\\n", \$\$1, \$\$2}'
EOF

  cat > "${output_dir}/payload/README.md" <<EOF
This directory holds the compiled WASM entrypoint for the plugin.

Run \`./build.sh\` (or \`make build\`) to compile \`src/lib.rs\` and place the
resulting \`.wasm\` file here as \`${entrypoint_rel}\`.

The filename must match the \`entrypoint\` field in \`plugin.json\`. The Kelvin
runtime verifies the file's SHA-256 against \`entrypoint_sha256\` in the manifest;
\`build.sh\` keeps that field up to date automatically.

\`.wasm\` files are excluded from version control (see \`.gitignore\`).
EOF

  cat > "${output_dir}/.gitignore" <<'EOF'
# Build artifacts — do not commit compiled WASM binaries or distribution archives.
/dist/
/target/
/payload/*.wasm
EOF

  cat > "${output_dir}/README.md" <<EOF
# ${display_name}

A Kelvin model provider plugin using the \`wasm_model_v1\` runtime.
Generated by \`scripts/kelvin-plugin.sh new --runtime wasm_model_v1\`.

## Architecture

Kelvin model plugins are thin WASM guests. The host (Kelvin runtime) calls the
guest's \`infer(ptr, len) -> i64\` export with a serialised \`ModelInput\` JSON.
Most plugins simply call the \`provider_profile_call\` host import, which tells
the host to make the actual HTTP request using the \`provider_profile\` declared
in \`plugin.json\`. All protocol translation, auth, and network I/O happen on the
host side — the WASM guest stays small and dependency-free.

## File Layout

| File             | Purpose                                                        |
|------------------|----------------------------------------------------------------|
| \`plugin.json\`    | Manifest: identity, provider_profile, capabilities, controls  |
| \`src/lib.rs\`     | Rust WASM guest: exports alloc/dealloc/infer, imports host fns|
| \`Cargo.toml\`     | Rust crate config (cdylib, no_std, wasm32 target)             |
| \`build.sh\`       | Compile, copy .wasm to payload/, update SHA-256 in plugin.json|
| \`Makefile\`       | Convenience targets: build, test, pack, install, smoke, clean |
| \`payload/\`       | Directory containing the compiled .wasm entrypoint            |

## Quick Start

\`\`\`bash
make build        # compile WASM and patch plugin.json SHA-256
make test         # validate manifest structure
make smoke        # run live inference (API key env var must be set)
make pack         # create dist/${plugin_id}-${plugin_version}.tar.gz
make install      # install into the local Kelvin plugin home
\`\`\`

## Customising provider_profile

The \`provider_profile\` object in \`plugin.json\` controls how the host routes
HTTP requests. Key fields:

| Field              | Description                                                  |
|--------------------|--------------------------------------------------------------|
| \`protocol_family\`  | \`openai_responses\`, \`anthropic_messages\`, or \`openai_chat_completions\` |
| \`api_key_env\`      | Environment variable holding the API key                    |
| \`base_url_env\`     | Environment variable to override the base URL (optional)    |
| \`default_base_url\` | Fallback base URL when the env var is unset                 |
| \`endpoint_path\`    | Appended to base URL (e.g. \`v1/messages\`)                   |
| \`auth_header\`      | Header name for the API key (\`authorization\` or \`x-api-key\`)|
| \`auth_scheme\`      | \`bearer\` (adds \`Bearer \` prefix) or \`raw\` (sends key as-is)  |
| \`static_headers\`   | Additional headers sent on every request (e.g. API versions)|
| \`default_allow_hosts\` | Must match \`capability_scopes.network_allow_hosts\`       |

## Customising the Guest

For most providers the passthrough \`infer()\` in \`src/lib.rs\` is sufficient.
For custom behaviour (input rewriting, caching, offline responses), see the
commented-out examples at the bottom of \`src/lib.rs\`.

### Available Host Imports (\`kelvin_model_host_v1\`)

| Import                  | Signature              | Purpose                                 |
|-------------------------|------------------------|-----------------------------------------|
| \`provider_profile_call\` | \`(i32, i32) -> i64\`  | Delegate to plugin.json provider_profile|
| \`openai_responses_call\` | \`(i32, i32) -> i64\`  | Hard-coded OpenAI Responses call        |
| \`log\`                   | \`(i32, i32, i32) -> i32\` | Log message (level 0–4, ptr, len)   |
| \`clock_now_ms\`          | \`() -> i64\`          | Current time in ms since Unix epoch     |

### ModelInput JSON (host → guest)

\`\`\`
{ "run_id", "session_id", "system_prompt", "user_prompt",
  "memory_snippets", "history", "tools" }
\`\`\`

### ModelOutput JSON (guest → host)

\`\`\`
{ "assistant_text", "stop_reason", "tool_calls", "usage" }
\`\`\`

## Testing

\`\`\`bash
make test         # static manifest validation (no API key needed)
make smoke        # live inference test (set API key env var first)
\`\`\`

## Packaging & Distribution

\`\`\`bash
make pack         # creates dist/${plugin_id}-${plugin_version}.tar.gz
make install      # installs from the packaged tarball
\`\`\`

Local development plugins can stay \`unsigned_local\`. Kelvin prints a warning
on install but still loads the plugin from a local plugin home.

To sign a plugin for distribution:

\`\`\`bash
scripts/plugin-sign.sh \\\\
  --manifest ./plugin.json \\\\
  --private-key /path/to/ed25519-private.pem \\\\
  --publisher-id your.publisher.id \\\\
  --trust-policy-out ./trusted_publishers.json
\`\`\`
EOF
}

usage() {
  cat <<'USAGE'
Usage: scripts/kelvin-plugin.sh <command> [options]

Commands:
  list      List installed plugins.
  search    Search the remote plugin index for available plugins.
  new       Create a new plugin package scaffold.
  test      Validate plugin manifest/layout and compatibility matrix.
  pack      Build a .tar.gz plugin package from manifest + payload.
  install   Install a local plugin package into a plugin home.
  index-install
            Install a published plugin package from a plugin index.
  verify    Verify package integrity and policy-tier requirements.
  smoke     Build, pack, install, and smoke-test a model plugin locally.

Run with --help after any command for command-specific options.
USAGE
}

cmd_list() {
  exec "${ROOT_DIR}/scripts/plugin-list.sh" "$@"
}

cmd_search() {
  exec "${ROOT_DIR}/scripts/plugin-discovery.sh" "$@"
}

new_usage() {
  cat <<'USAGE'
Usage: scripts/kelvin-plugin.sh new [options]

Options:
  --id <plugin-id>          Required plugin id (example: acme.echo)
  --name <display-name>     Required plugin name
  --version <semver>        Plugin version (default: 0.1.0)
  --runtime <kind>          wasm_tool_v1 or wasm_model_v1 (default: wasm_tool_v1)
  --out <dir>               Output directory (default: ./plugin-<id>)
  --tool-name <name>        Tool runtime: tool name (default: derived from id)
  --provider-name <name>    Model runtime: provider name (default: derived from profile or id)
  --provider-profile <id>   Model runtime: provider_profile.id (default: openai.responses)
  --protocol-family <name>  Model runtime: openai_responses|openai_chat_completions|anthropic_messages
  --api-key-env <name>      Model runtime: API key environment variable
  --base-url-env <name>     Model runtime: base URL override environment variable
  --default-base-url <url>  Model runtime: default provider base URL
  --endpoint-path <path>    Model runtime: relative endpoint path (example: v1/responses)
  --auth-header <name>      Model runtime: auth header name (default: authorization)
  --auth-scheme <name>      Model runtime: bearer|raw (default: bearer)
  --allow-host <host>       Model runtime: allowed host pattern (repeatable)
  --no-api-key              Model runtime: skip API key requirement (for unauthenticated providers)
  --dynamic-base-url        Model runtime: derive allowed host from OLLAMA_BASE_URL at runtime
  --model-name <name>       Model runtime: model name (default: protocol-family default)
  --entrypoint <path>       Relative wasm payload path (default: plugin.wasm)
  --quality-tier <tier>     unsigned_local|signed_community|signed_trusted (default: unsigned_local)
  --force                   Overwrite an existing non-empty output directory

`wasm_model_v1` scaffolds emit a structured `provider_profile` object, create a
Rust guest source project, and run a local build, so `cargo`, `rustup`, and
`jq` must be available.
USAGE
}

install_usage() {
  cat <<'USAGE'
Usage: scripts/kelvin-plugin.sh install --package <plugin-package.tar.gz> [options]

Options:
  --package <path>          Plugin package tarball (.tar.gz)
  --plugin-home <dir>       Install root (default: $KELVIN_PLUGIN_HOME or ~/.kelvinclaw/plugins)
  --force                   Overwrite existing plugin version
USAGE
}

index_install_usage() {
  cat <<'USAGE'
Usage: scripts/kelvin-plugin.sh index-install --plugin <id> [options]

Options:
  --plugin <id>             Plugin id from index
  --version <version>       Specific version to install
  --index-url <url>         Plugin index JSON URL
  --registry-url <url>      Hosted registry base URL (uses /v1/index.json)
  --plugin-home <dir>       Install root (default: $KELVIN_PLUGIN_HOME or ~/.kelvinclaw/plugins)
  --trust-policy-path <path>
                            Trust policy file to merge/update
  --force                   Reinstall even if version exists
  --min-quality-tier <tier> unsigned_local|signed_community|signed_trusted
USAGE
}

test_usage() {
  cat <<'USAGE'
Usage: scripts/kelvin-plugin.sh test --manifest <plugin.json> [options]

Options:
  --manifest <path>         Required path to plugin.json
  --core-versions <csv>     Core versions matrix (default: 0.1.0)
  --core-api-version <semver>
                            Core API semver (default: 1.0.0)
  --json                    Emit machine-readable output JSON
USAGE
}

pack_usage() {
  cat <<'USAGE'
Usage: scripts/kelvin-plugin.sh pack --manifest <plugin.json> [options]

Options:
  --manifest <path>         Required path to plugin.json
  --output <path>           Output .tar.gz path (default: ./dist/<id>-<version>.tar.gz)
  --core-versions <csv>     Core versions matrix for pre-pack validation
USAGE
}

verify_usage() {
  cat <<'USAGE'
Usage: scripts/kelvin-plugin.sh verify [options]

Options:
  --package <path>          Plugin package tarball (.tar.gz)
  --manifest <path>         Plugin manifest path (if package is omitted)
  --trust-policy <path>     Trust policy file for signed_trusted checks
  --core-versions <csv>     Core versions matrix (default: 0.1.0)
  --json                    Emit machine-readable output JSON

Note: you must pass either --package or --manifest.
USAGE
}

smoke_usage() {
  cat <<'USAGE'
Usage: scripts/kelvin-plugin.sh smoke --manifest <plugin.json> [options]

Options:
  --manifest <path>         Required path to plugin.json
  --plugin-home <dir>       Plugin home to install into (default: temporary)
  --trust-policy <path>     Trust policy path for CLI plugin install (default: temporary)
  --workspace <dir>         Workspace directory for kelvin-host (default: manifest directory)
  --prompt <text>           Prompt for the smoke run
                            (default: Say hello in one sentence.)
  --core-versions <csv>     Core versions matrix for validation (default: 0.1.0)
  --skip-cli-install        Do not auto-install kelvin.cli from the plugin index
  --no-build                Skip running ./build.sh before packing
  --keep-temp               Keep temporary smoke artifacts on disk
  --json                    Emit machine-readable result JSON

Behavior:
  - If build.sh exists next to plugin.json, Kelvin runs it unless --no-build is set.
  - Kelvin packs and installs the plugin locally.
  - Kelvin auto-installs kelvin.cli unless --skip-cli-install is set.
  - If provider_profile.api_key_env is unset, a clear "<ENV> is required" failure
    is treated as a successful no-key smoke.
USAGE
}

validate_manifest_and_layout() {
  local manifest_path="$1"
  local core_versions_csv="$2"
  local core_api_version="$3"
  local json_output="${4:-0}"

  require_cmd jq

  if [[ ! -f "${manifest_path}" ]]; then
    echo "Manifest not found: ${manifest_path}" >&2
    return 1
  fi

  local manifest_dir
  manifest_dir="$(cd "$(dirname "${manifest_path}")" && pwd)"
  local payload_dir="${manifest_dir}/payload"
  local core_api_major
  core_api_major="$(cut -d'.' -f1 <<< "${core_api_version}")"

  local id name version api_version runtime entrypoint capability_count quality_tier
  id="$(jq -er '.id' "${manifest_path}")"
  name="$(jq -er '.name' "${manifest_path}")"
  version="$(jq -er '.version' "${manifest_path}")"
  api_version="$(jq -er '.api_version' "${manifest_path}")"
  runtime="$(jq -er '.runtime // "wasm_tool_v1"' "${manifest_path}")"
  entrypoint="$(jq -er '.entrypoint' "${manifest_path}")"
  capability_count="$(jq -er '.capabilities | length' "${manifest_path}")"
  quality_tier="$(jq -er '.quality_tier // "unsigned_local"' "${manifest_path}")"

  [[ "${id}" =~ ^[A-Za-z0-9._-]{1,128}$ ]] || {
    echo "Invalid plugin id '${id}'" >&2
    return 1
  }
  [[ -n "${name// }" ]] || {
    echo "Plugin name must not be empty" >&2
    return 1
  }
  semver_valid "${version}" || {
    echo "Plugin version must be semver: ${version}" >&2
    return 1
  }
  semver_valid "${api_version}" || {
    echo "Plugin api_version must be semver: ${api_version}" >&2
    return 1
  }
  quality_tier_valid "${quality_tier}" || {
    echo "Invalid quality_tier '${quality_tier}'" >&2
    return 1
  }
  [[ "${capability_count}" -ge 1 ]] || {
    echo "Manifest capabilities must contain at least one value" >&2
    return 1
  }

  case "${runtime}" in
    wasm_tool_v1|wasm_model_v1) ;;
    *)
      echo "Unsupported runtime '${runtime}'" >&2
      return 1
      ;;
  esac

  if [[ "${entrypoint}" == /* || "${entrypoint}" == *".."* ]]; then
    echo "Manifest entrypoint must be a safe relative path" >&2
    return 1
  fi

  local entrypoint_abs="${payload_dir}/${entrypoint}"
  if [[ ! -f "${entrypoint_abs}" ]]; then
    echo "Entrypoint file missing: ${entrypoint_abs}" >&2
    return 1
  fi

  local expected_sha actual_sha
  expected_sha="$(jq -er '.entrypoint_sha256 // ""' "${manifest_path}")"
  if [[ -n "${expected_sha}" ]]; then
    actual_sha="$(sha256_file "${entrypoint_abs}")"
    if [[ "${actual_sha}" != "${expected_sha}" ]]; then
      echo "entrypoint_sha256 mismatch (expected=${expected_sha} actual=${actual_sha})" >&2
      return 1
    fi
  fi

  if [[ "${runtime}" == "wasm_tool_v1" ]]; then
    jq -e '.capabilities | index("tool_provider") != null' "${manifest_path}" >/dev/null || {
      echo "wasm_tool_v1 requires capability 'tool_provider'" >&2
      return 1
    }
    jq -e '.tool_name | type=="string" and length>0' "${manifest_path}" >/dev/null || {
      echo "wasm_tool_v1 requires non-empty tool_name" >&2
      return 1
    }
  fi

  if [[ "${runtime}" == "wasm_model_v1" ]]; then
    jq -e '.capabilities | index("model_provider") != null' "${manifest_path}" >/dev/null || {
      echo "wasm_model_v1 requires capability 'model_provider'" >&2
      return 1
    }
    jq -e '
      ((.provider_name == null) or (.provider_name | type=="string" and length>0)) and
      (.provider_profile | type=="object") and
      (.provider_profile.id | type=="string" and length>0) and
      (.provider_profile.provider_name | type=="string" and length>0) and
      ((.provider_name == null) or (.provider_name == .provider_profile.provider_name)) and
      (.provider_profile.protocol_family | type=="string" and (. == "openai_responses" or . == "openai_chat_completions" or . == "anthropic_messages")) and
      ((.provider_profile.api_key_env == null) or (.provider_profile.api_key_env | type=="string" and length>0)) and
      (.provider_profile.base_url_env | type=="string" and length>0) and
      (.provider_profile.default_base_url | type=="string" and length>0) and
      (.provider_profile.endpoint_path | type=="string" and length>0) and
      (.provider_profile.auth_header | type=="string" and length>0) and
      (.provider_profile.auth_scheme | type=="string" and (. == "bearer" or . == "raw")) and
      (.provider_profile.static_headers | type=="array") and
      ([.provider_profile.static_headers[]? | (.name | type=="string" and length>0) and (.value | type=="string" and length>0)] | all) and
      (.provider_profile.default_allow_hosts | type=="array") and
      ((.provider_profile.dynamic_base_url == true) or (.provider_profile.default_allow_hosts | length>0)) and
      ([.provider_profile.default_allow_hosts[] | type=="string" and length>0] | all)
    ' "${manifest_path}" >/dev/null || {
      echo "wasm_model_v1 requires a structured provider_profile object" >&2
      return 1
    }
    jq -e '
      (.provider_profile.dynamic_base_url == true) or
      (.capability_scopes.network_allow_hosts | type=="array" and length>0 and ([.[] | type=="string" and length>0] | all))
    ' "${manifest_path}" >/dev/null || {
      echo "wasm_model_v1 requires non-empty capability_scopes.network_allow_hosts (or dynamic_base_url: true)" >&2
      return 1
    }
    jq -e '.model_name | type=="string" and length>0' "${manifest_path}" >/dev/null || {
      echo "wasm_model_v1 requires non-empty model_name" >&2
      return 1
    }
  fi

  local plugin_api_major
  plugin_api_major="$(cut -d'.' -f1 <<< "${api_version}")"
  if [[ "${plugin_api_major}" != "${core_api_major}" ]]; then
    echo "api major mismatch: plugin=${plugin_api_major} core=${core_api_major}" >&2
    return 1
  fi

  local min_core max_core
  min_core="$(jq -er '.min_core_version // ""' "${manifest_path}")"
  max_core="$(jq -er '.max_core_version // ""' "${manifest_path}")"
  if [[ -n "${min_core}" ]]; then
    semver_valid "${min_core}" || {
      echo "min_core_version must be semver" >&2
      return 1
    }
  fi
  if [[ -n "${max_core}" ]]; then
    semver_valid "${max_core}" || {
      echo "max_core_version must be semver" >&2
      return 1
    }
  fi

  local compatibility="[]"
  local core_version
  IFS=',' read -r -a _versions <<< "${core_versions_csv}"
  for core_version in "${_versions[@]}"; do
    core_version="$(xargs <<< "${core_version}")"
    [[ -n "${core_version}" ]] || continue
    semver_valid "${core_version}" || {
      echo "core version '${core_version}' is not semver" >&2
      return 1
    }
    local compatible="true"
    local reason="ok"
    if [[ -n "${min_core}" ]] && ! semver_ge "${core_version}" "${min_core}"; then
      compatible="false"
      reason="below_min_core_version"
    fi
    if [[ -n "${max_core}" ]] && ! semver_le "${core_version}" "${max_core}"; then
      compatible="false"
      reason="above_max_core_version"
    fi
    compatibility="$(
      jq -cn \
        --argjson existing "${compatibility}" \
        --arg version "${core_version}" \
        --arg compatible "${compatible}" \
        --arg reason "${reason}" \
        '$existing + [{core_version:$version, compatible:($compatible=="true"), reason:$reason}]'
    )"
  done

  if [[ "${json_output}" == "1" ]]; then
    jq -cn \
      --arg id "${id}" \
      --arg name "${name}" \
      --arg version "${version}" \
      --arg runtime "${runtime}" \
      --arg entrypoint "${entrypoint}" \
      --arg quality_tier "${quality_tier}" \
      --argjson compatibility "${compatibility}" \
      '{
        id:$id,
        name:$name,
        version:$version,
        runtime:$runtime,
        entrypoint:$entrypoint,
        quality_tier:$quality_tier,
        compatibility:$compatibility
      }'
  else
    echo "[kelvin-plugin] manifest ok: ${id}@${version} (${runtime})"
    echo "[kelvin-plugin] compatibility matrix:"
    jq -r '.[] | "  - core=\(.core_version) compatible=\(.compatible) reason=\(.reason)"' <<< "${compatibility}"
  fi
}

cmd_new() {
  local id="" name="" version="0.1.0" runtime="wasm_tool_v1" out="" tool_name=""
  local provider_name="" provider_profile_id="" protocol_family="" api_key_env="" base_url_env=""
  local default_base_url="" endpoint_path="" auth_header="" auth_scheme="bearer"
  local model_name="default" entrypoint="plugin.wasm" quality_tier="unsigned_local"
  local force="0" no_api_key="0" dynamic_base_url="0"
  local -a allow_hosts=()

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --id) id="${2:?missing value for --id}"; shift 2 ;;
      --name) name="${2:?missing value for --name}"; shift 2 ;;
      --version) version="${2:?missing value for --version}"; shift 2 ;;
      --runtime) runtime="${2:?missing value for --runtime}"; shift 2 ;;
      --out) out="${2:?missing value for --out}"; shift 2 ;;
      --tool-name) tool_name="${2:?missing value for --tool-name}"; shift 2 ;;
      --provider-name) provider_name="${2:?missing value for --provider-name}"; shift 2 ;;
      --provider-profile) provider_profile_id="${2:?missing value for --provider-profile}"; shift 2 ;;
      --protocol-family) protocol_family="${2:?missing value for --protocol-family}"; shift 2 ;;
      --api-key-env) api_key_env="${2:?missing value for --api-key-env}"; shift 2 ;;
      --base-url-env) base_url_env="${2:?missing value for --base-url-env}"; shift 2 ;;
      --default-base-url) default_base_url="${2:?missing value for --default-base-url}"; shift 2 ;;
      --endpoint-path) endpoint_path="${2:?missing value for --endpoint-path}"; shift 2 ;;
      --auth-header) auth_header="${2:?missing value for --auth-header}"; shift 2 ;;
      --auth-scheme) auth_scheme="${2:?missing value for --auth-scheme}"; shift 2 ;;
      --allow-host) allow_hosts+=("${2:?missing value for --allow-host}"); shift 2 ;;
      --model-name) model_name="${2:?missing value for --model-name}"; shift 2 ;;
      --entrypoint) entrypoint="${2:?missing value for --entrypoint}"; shift 2 ;;
      --quality-tier) quality_tier="${2:?missing value for --quality-tier}"; shift 2 ;;
      --force) force="1"; shift ;;
      --no-api-key) no_api_key="1"; shift ;;
      --dynamic-base-url) dynamic_base_url="1"; shift ;;
      -h|--help) new_usage; exit 0 ;;
      *) echo "Unknown argument: $1" >&2; new_usage; exit 1 ;;
    esac
  done

  [[ -n "${id}" && -n "${name}" ]] || {
    echo "--id and --name are required" >&2
    new_usage
    exit 1
  }
  semver_valid "${version}" || {
    echo "--version must be semver" >&2
    exit 1
  }
  quality_tier_valid "${quality_tier}" || {
    echo "Invalid --quality-tier '${quality_tier}'" >&2
    exit 1
  }
  case "${runtime}" in
    wasm_tool_v1|wasm_model_v1) ;;
    *) echo "Unsupported --runtime '${runtime}'" >&2; exit 1 ;;
  esac

  if [[ -z "${out}" ]]; then
    out="./plugin-${id}"
  fi
  if [[ -e "${out}" && "${force}" != "1" ]]; then
    if [[ -d "${out}" ]] && [[ -n "$(find "${out}" -mindepth 1 -maxdepth 1 -print -quit 2>/dev/null)" ]]; then
      echo "Refusing to overwrite non-empty output directory: ${out}" >&2
      echo "Re-run with --force if you want to replace it." >&2
      exit 1
    fi
    if [[ ! -d "${out}" ]]; then
      echo "Refusing to overwrite existing path: ${out}" >&2
      echo "Re-run with --force if you want to replace it." >&2
      exit 1
    fi
  fi
  if [[ "${force}" == "1" && -e "${out}" ]]; then
    rm -rf "${out}"
  fi
  mkdir -p "${out}/payload"
  if [[ -z "${tool_name}" ]]; then
    tool_name="$(tr '.-' '_' <<< "${id}")"
  fi

  local capabilities runtime_extra network_allow_hosts timeout_ms crate_package_name crate_lib_name
  if [[ "${runtime}" == "wasm_model_v1" ]]; then
    local builtin_profile_json="" provider_profile_json=""
    if [[ -z "${provider_profile_id}" ]]; then
      provider_profile_id="openai.responses"
    fi
    builtin_profile_json="$(builtin_provider_profile_json "${provider_profile_id}" 2>/dev/null || true)"
    if [[ -n "${builtin_profile_json}" ]]; then
      [[ -n "${protocol_family}" ]] || protocol_family="$(jq -er '.protocol_family' <<< "${builtin_profile_json}")"
      [[ -n "${provider_name}" ]] || provider_name="$(jq -er '.provider_name' <<< "${builtin_profile_json}")"
      [[ -n "${api_key_env}" ]] || api_key_env="$(jq -er '.api_key_env' <<< "${builtin_profile_json}")"
      [[ -n "${base_url_env}" ]] || base_url_env="$(jq -er '.base_url_env' <<< "${builtin_profile_json}")"
      [[ -n "${default_base_url}" ]] || default_base_url="$(jq -er '.default_base_url' <<< "${builtin_profile_json}")"
      [[ -n "${endpoint_path}" ]] || endpoint_path="$(jq -er '.endpoint_path' <<< "${builtin_profile_json}")"
      [[ -n "${auth_header}" ]] || auth_header="$(jq -er '.auth_header' <<< "${builtin_profile_json}")"
      [[ "${auth_scheme}" != "bearer" ]] || auth_scheme="$(jq -er '.auth_scheme' <<< "${builtin_profile_json}")"
      if [[ "${#allow_hosts[@]}" -eq 0 ]]; then
        while IFS= read -r _host; do
          allow_hosts+=("$_host")
        done < <(jq -r '.default_allow_hosts[]' <<< "${builtin_profile_json}")
      fi
    fi
    [[ -n "${protocol_family}" ]] || {
      echo "wasm_model_v1 requires --protocol-family for non-builtin provider profiles" >&2
      exit 1
    }
    [[ -n "${provider_name}" ]] || provider_name="$(tr '.-' '_' <<< "${id}")"
    [[ "${no_api_key}" == "1" || -n "${api_key_env}" ]] || {
      echo "wasm_model_v1 requires --api-key-env (or --no-api-key for unauthenticated providers)" >&2
      exit 1
    }
    [[ -n "${base_url_env}" ]] || {
      echo "wasm_model_v1 requires --base-url-env" >&2
      exit 1
    }
    [[ -n "${auth_header}" ]] || auth_header="authorization"
    [[ -n "${default_base_url}" ]] || {
      echo "wasm_model_v1 requires --default-base-url" >&2
      exit 1
    }
    [[ -n "${endpoint_path}" ]] || {
      echo "wasm_model_v1 requires --endpoint-path" >&2
      exit 1
    }
    [[ "${auth_scheme}" == "bearer" || "${auth_scheme}" == "raw" ]] || {
      echo "wasm_model_v1 --auth-scheme must be bearer or raw" >&2
      exit 1
    }
    [[ "${dynamic_base_url}" == "1" || "${#allow_hosts[@]}" -gt 0 ]] || {
      echo "wasm_model_v1 requires at least one --allow-host (or --dynamic-base-url for user-configured hosts)" >&2
      exit 1
    }
    if [[ "${model_name}" == "default" ]]; then
      model_name="$(protocol_family_default_model_name "${protocol_family}" "${provider_name}")"
    fi
    if [[ "${#allow_hosts[@]}" -gt 0 ]]; then
      network_allow_hosts="$(printf '%s\n' "${allow_hosts[@]}" | jq -R . | jq -s .)"
    else
      network_allow_hosts='[]'
    fi
    timeout_ms="30000"
    capabilities='["model_provider","network_egress"]'
    local api_key_env_json
    if [[ "${no_api_key}" == "1" ]]; then
      api_key_env_json="null"
    else
      api_key_env_json="$(jq -cn --arg v "${api_key_env}" '$v')"
    fi
    provider_profile_json="$(jq -cn \
      --arg id "${provider_profile_id}" \
      --arg provider_name "${provider_name}" \
      --arg protocol_family "${protocol_family}" \
      --argjson api_key_env "${api_key_env_json}" \
      --arg base_url_env "${base_url_env}" \
      --arg default_base_url "${default_base_url}" \
      --arg endpoint_path "${endpoint_path}" \
      --arg auth_header "${auth_header}" \
      --arg auth_scheme "${auth_scheme}" \
      --argjson default_allow_hosts "${network_allow_hosts}" \
      --argjson dynamic_base_url "$([ "${dynamic_base_url}" == "1" ] && echo 'true' || echo 'false')" \
      '{
        id:$id,
        provider_name:$provider_name,
        protocol_family:$protocol_family,
        api_key_env:$api_key_env,
        base_url_env:$base_url_env,
        default_base_url:$default_base_url,
        endpoint_path:$endpoint_path,
        auth_header:$auth_header,
        auth_scheme:$auth_scheme,
        static_headers:[],
        default_allow_hosts:$default_allow_hosts,
        dynamic_base_url:$dynamic_base_url
      }')"
    if [[ -n "${builtin_profile_json}" ]]; then
      provider_profile_json="$(jq -cn \
        --argjson builtin "${builtin_profile_json}" \
        --argjson overrides "${provider_profile_json}" \
        '$builtin * $overrides')"
    fi
    runtime_extra="$(jq -cn \
      --arg provider_name "${provider_name}" \
      --arg model_name "${model_name}" \
      --argjson provider_profile "${provider_profile_json}" \
      '{
        provider_name:$provider_name,
        provider_profile:$provider_profile,
        model_name:$model_name
      }')"
  else
    network_allow_hosts='[]'
    timeout_ms="2000"
    capabilities='["tool_provider"]'
    runtime_extra="$(jq -cn --arg tool_name "${tool_name}" '{
      tool_name:$tool_name,
      tool_input_schema:{
        type:"object",
        properties:{},
        required:[]
      }
    }')"
  fi

  jq -cn \
    --arg id "${id}" \
    --arg name "${name}" \
    --arg version "${version}" \
    --arg runtime "${runtime}" \
    --arg entrypoint "${entrypoint}" \
    --arg quality_tier "${quality_tier}" \
    --argjson network_allow_hosts "${network_allow_hosts}" \
    --argjson timeout_ms "${timeout_ms}" \
    --argjson capabilities "${capabilities}" \
    --argjson runtime_extra "${runtime_extra}" \
    '{
      id:$id,
      name:$name,
      version:$version,
      api_version:"1.0.0",
      description:"Kelvin plugin scaffold",
      homepage:"https://github.com/agentichighway/kelvinclaw",
      capabilities:$capabilities,
      experimental:false,
      min_core_version:"0.1.0",
      max_core_version:null,
      runtime:$runtime,
      entrypoint:$entrypoint,
      entrypoint_sha256:null,
      publisher:null,
      quality_tier:$quality_tier,
      capability_scopes:{
        fs_read_paths:[],
        network_allow_hosts:$network_allow_hosts
      },
      operational_controls:{
        timeout_ms:$timeout_ms,
        max_retries:0,
        max_calls_per_minute:120,
        circuit_breaker_failures:3,
        circuit_breaker_cooldown_ms:30000
      }
    } + $runtime_extra' > "${out}/plugin.json"

  if [[ "${runtime}" == "wasm_model_v1" ]]; then
    crate_package_name="$(tr '._' '-' <<< "${id}")-plugin"
    crate_lib_name="$(tr '.-' '_' <<< "${id}")_plugin"
    scaffold_model_plugin_project "${out}" "${id}" "${name}" "${version}" "${entrypoint}" "${crate_package_name}" "${crate_lib_name}"
    (
      cd "${out}"
      ./build.sh >/dev/null
    )
  else
    crate_package_name="$(tr '._' '-' <<< "${id}")-plugin"
    crate_lib_name="$(tr '.-' '_' <<< "${id}")_plugin"
    scaffold_tool_plugin_project "${out}" "${id}" "${name}" "${version}" "${entrypoint}" "${crate_package_name}" "${crate_lib_name}"
    (
      cd "${out}"
      ./build.sh >/dev/null
    )
  fi

  echo "[kelvin-plugin] scaffold created at ${out}"
}

cmd_test() {
  local manifest="" core_versions="${DEFAULT_CORE_VERSIONS}" core_api_version="${DEFAULT_CORE_API_VERSION}" json_output="0"
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --manifest) manifest="${2:?missing value for --manifest}"; shift 2 ;;
      --core-versions) core_versions="${2:?missing value for --core-versions}"; shift 2 ;;
      --core-api-version) core_api_version="${2:?missing value for --core-api-version}"; shift 2 ;;
      --json) json_output="1"; shift ;;
      -h|--help) test_usage; exit 0 ;;
      *) echo "Unknown argument: $1" >&2; test_usage; exit 1 ;;
    esac
  done
  [[ -n "${manifest}" ]] || {
    echo "--manifest is required" >&2
    test_usage
    exit 1
  }
  validate_manifest_and_layout "${manifest}" "${core_versions}" "${core_api_version}" "${json_output}"
}

cmd_pack() {
  local manifest="" output="" core_versions="${DEFAULT_CORE_VERSIONS}"
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --manifest) manifest="${2:?missing value for --manifest}"; shift 2 ;;
      --output) output="${2:?missing value for --output}"; shift 2 ;;
      --core-versions) core_versions="${2:?missing value for --core-versions}"; shift 2 ;;
      -h|--help) pack_usage; exit 0 ;;
      *) echo "Unknown argument: $1" >&2; pack_usage; exit 1 ;;
    esac
  done
  [[ -n "${manifest}" ]] || {
    echo "--manifest is required" >&2
    pack_usage
    exit 1
  }
  validate_manifest_and_layout "${manifest}" "${core_versions}" "${DEFAULT_CORE_API_VERSION}" "0"

  local manifest_dir
  manifest_dir="$(cd "$(dirname "${manifest}")" && pwd)"
  local id version
  id="$(jq -er '.id' "${manifest}")"
  version="$(jq -er '.version' "${manifest}")"

  if [[ -z "${output}" ]]; then
    mkdir -p "${manifest_dir}/dist"
    output="${manifest_dir}/dist/${id}-${version}.tar.gz"
  fi
  mkdir -p "$(dirname "${output}")"

  local include_sig=""
  if [[ -f "${manifest_dir}/plugin.sig" ]]; then
    include_sig="plugin.sig"
  fi
  create_tar_gz "${output}" "${manifest_dir}" plugin.json payload ${include_sig}
  echo "[kelvin-plugin] package created: ${output}"
}

cmd_verify() {
  local package="" manifest="" trust_policy="" core_versions="${DEFAULT_CORE_VERSIONS}" json_output="0"
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --package) package="${2:?missing value for --package}"; shift 2 ;;
      --manifest) manifest="${2:?missing value for --manifest}"; shift 2 ;;
      --trust-policy) trust_policy="${2:?missing value for --trust-policy}"; shift 2 ;;
      --core-versions) core_versions="${2:?missing value for --core-versions}"; shift 2 ;;
      --json) json_output="1"; shift ;;
      -h|--help) verify_usage; exit 0 ;;
      *) echo "Unknown argument: $1" >&2; verify_usage; exit 1 ;;
    esac
  done

  local work_dir
  work_dir="$(mktemp -d)"
  trap "rm -rf '${work_dir}'" EXIT

  if [[ -n "${package}" ]]; then
    [[ -f "${package}" ]] || {
      echo "Package not found: ${package}" >&2
      exit 1
    }
    tar -xzf "${package}" -C "${work_dir}"
    manifest="${work_dir}/plugin.json"
  fi
  [[ -n "${manifest}" ]] || {
    echo "Provide either --package or --manifest" >&2
    verify_usage
    exit 1
  }

  validate_manifest_and_layout "${manifest}" "${core_versions}" "${DEFAULT_CORE_API_VERSION}" "0"
  local manifest_dir quality_tier publisher sig_path
  manifest_dir="$(cd "$(dirname "${manifest}")" && pwd)"
  quality_tier="$(jq -er '.quality_tier // "unsigned_local"' "${manifest}")"
  publisher="$(jq -er '.publisher // ""' "${manifest}")"
  sig_path="${manifest_dir}/plugin.sig"

  case "${quality_tier}" in
    unsigned_local) ;;
    signed_community|signed_trusted)
      [[ -f "${sig_path}" ]] || {
        echo "quality_tier=${quality_tier} requires plugin.sig" >&2
        exit 1
      }
      [[ -n "${publisher}" ]] || {
        echo "quality_tier=${quality_tier} requires non-empty publisher" >&2
        exit 1
      }
      ;;
  esac

  if [[ "${quality_tier}" == "signed_trusted" ]]; then
    [[ -n "${trust_policy}" ]] || {
      echo "signed_trusted verification requires --trust-policy" >&2
      exit 1
    }
    [[ -f "${trust_policy}" ]] || {
      echo "Trust policy not found: ${trust_policy}" >&2
      exit 1
    }
    jq -e --arg publisher "${publisher}" '
      (.publishers // []) | any(.id == $publisher)
    ' "${trust_policy}" >/dev/null || {
      echo "publisher '${publisher}' not present in trust policy" >&2
      exit 1
    }
    jq -e --arg publisher "${publisher}" '
      ((.revoked_publishers // []) | index($publisher)) | not
    ' "${trust_policy}" >/dev/null || {
      echo "publisher '${publisher}' is revoked in trust policy" >&2
      exit 1
    }
  fi

  if [[ -n "${package}" ]]; then
    local dry_home="${work_dir}/dry-home"
    KELVIN_PLUGIN_HOME="${dry_home}" "${ROOT_DIR}/scripts/plugin-install.sh" --package "${package}" >/dev/null
  fi

  if [[ "${json_output}" == "1" ]]; then
    jq -cn \
      --arg manifest "${manifest}" \
      --arg quality_tier "${quality_tier}" \
      --arg publisher "${publisher}" \
      '{"verified":true,"manifest":$manifest,"quality_tier":$quality_tier,"publisher":(if $publisher=="" then null else $publisher end)}'
  else
    echo "[kelvin-plugin] verify ok (${quality_tier})"
  fi
}

cmd_install() {
  local -a args=()
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --package|--plugin-home)
        args+=("$1" "${2:?missing value for $1}")
        shift 2
        ;;
      --force)
        args+=("$1")
        shift
        ;;
      -h|--help)
        install_usage
        exit 0
        ;;
      *)
        echo "Unknown argument: $1" >&2
        install_usage
        exit 1
        ;;
    esac
  done
  exec "${ROOT_DIR}/scripts/plugin-install.sh" "${args[@]}"
}

cmd_index_install() {
  local -a args=()
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --plugin|--version|--index-url|--registry-url|--plugin-home|--trust-policy-path|--min-quality-tier)
        args+=("$1" "${2:?missing value for $1}")
        shift 2
        ;;
      --force)
        args+=("$1")
        shift
        ;;
      -h|--help)
        index_install_usage
        exit 0
        ;;
      *)
        echo "Unknown argument: $1" >&2
        index_install_usage
        exit 1
        ;;
    esac
  done
  exec "${ROOT_DIR}/scripts/plugin-index-install.sh" "${args[@]}"
}

cmd_smoke() {
  local manifest="" plugin_home="" trust_policy="" workspace="" prompt="Say hello in one sentence."
  local core_versions="${DEFAULT_CORE_VERSIONS}" skip_cli_install="0" no_build="0" keep_temp="0" json_output="0"

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --manifest) manifest="${2:?missing value for --manifest}"; shift 2 ;;
      --plugin-home) plugin_home="${2:?missing value for --plugin-home}"; shift 2 ;;
      --trust-policy) trust_policy="${2:?missing value for --trust-policy}"; shift 2 ;;
      --workspace) workspace="${2:?missing value for --workspace}"; shift 2 ;;
      --prompt) prompt="${2:?missing value for --prompt}"; shift 2 ;;
      --core-versions) core_versions="${2:?missing value for --core-versions}"; shift 2 ;;
      --skip-cli-install) skip_cli_install="1"; shift ;;
      --no-build) no_build="1"; shift ;;
      --keep-temp) keep_temp="1"; shift ;;
      --json) json_output="1"; shift ;;
      -h|--help) smoke_usage; exit 0 ;;
      *)
        echo "Unknown argument: $1" >&2
        smoke_usage
        exit 1
        ;;
    esac
  done

  [[ -n "${manifest}" ]] || {
    echo "--manifest is required" >&2
    smoke_usage
    exit 1
  }

  require_cmd cargo
  if [[ "${skip_cli_install}" != "1" ]]; then
    require_cmd curl
  fi

  validate_manifest_and_layout "${manifest}" "${core_versions}" "${DEFAULT_CORE_API_VERSION}" "0"

  local manifest_dir runtime plugin_id api_key_env temp_dir package_path host_output
  local keep_temp_value=""
  manifest_dir="$(cd "$(dirname "${manifest}")" && pwd)"
  runtime="$(jq -er '.runtime // "wasm_tool_v1"' "${manifest}")"
  plugin_id="$(jq -er '.id' "${manifest}")"
  if [[ "${runtime}" != "wasm_model_v1" ]]; then
    echo "smoke currently supports wasm_model_v1 plugins only" >&2
    exit 1
  fi
  api_key_env="$(jq -r '.provider_profile.api_key_env // ""' "${manifest}")"

  temp_dir="$(mktemp -d)"
  keep_temp_value="${keep_temp}"
  trap 'if [[ "'"${keep_temp_value}"'" != "1" ]]; then rm -rf "'"${temp_dir}"'"; fi' EXIT

  if [[ -z "${plugin_home}" ]]; then
    plugin_home="${temp_dir}/plugins"
  fi
  if [[ -z "${trust_policy}" ]]; then
    trust_policy="${temp_dir}/trusted_publishers.json"
  fi
  if [[ -z "${workspace}" ]]; then
    workspace="${manifest_dir}"
  fi
  package_path="${temp_dir}/$(jq -er '.id' "${manifest}")-$(jq -er '.version' "${manifest}").tar.gz"
  host_output="${temp_dir}/kelvin-host.log"

  mkdir -p "${plugin_home}" "$(dirname "${trust_policy}")"

  if [[ "${no_build}" != "1" && -x "${manifest_dir}/build.sh" ]]; then
    (
      cd "${manifest_dir}"
      ./build.sh >/dev/null
    )
  fi

  cmd_pack --manifest "${manifest}" --output "${package_path}" --core-versions "${core_versions}" >/dev/null
  "${ROOT_DIR}/scripts/plugin-install.sh" --package "${package_path}" --plugin-home "${plugin_home}" --force >/dev/null

  if [[ "${skip_cli_install}" != "1" && ! -d "${plugin_home}/kelvin.cli/current" ]]; then
    "${ROOT_DIR}/scripts/plugin-index-install.sh" \
      --plugin kelvin.cli \
      --plugin-home "${plugin_home}" \
      --trust-policy-path "${trust_policy}" \
      --min-quality-tier signed_trusted \
      --force >/dev/null
  fi

  set +e
  (
    cd "${ROOT_DIR}"
    KELVIN_PLUGIN_HOME="${plugin_home}" \
    KELVIN_TRUST_POLICY_PATH="${trust_policy}" \
    cargo run -q -p kelvin-host -- \
      --prompt "${prompt}" \
      --workspace "${workspace}" \
      --memory fallback \
      --model-provider "${plugin_id}"
  ) >"${host_output}" 2>&1
  local status=$?
  set -e

  local key_present="0"
  [[ -n "${api_key_env}" && -n "${!api_key_env:-}" ]] && key_present="1"

  if [[ -n "${api_key_env}" && "${key_present}" == "0" ]]; then
    # Plugin requires an API key but it is not set — expect a clear error message.
    if grep -Fq "${api_key_env} is required" "${host_output}"; then
      if [[ "${json_output}" == "1" ]]; then
        jq -cn \
          --arg plugin_id "${plugin_id}" \
          --arg api_key_env "${api_key_env}" \
          --arg plugin_home "${plugin_home}" \
          --arg trust_policy "${trust_policy}" \
          --arg workspace "${workspace}" \
          '{"ok":true,"mode":"missing_key_expected","plugin_id":$plugin_id,"api_key_env":$api_key_env,"plugin_home":$plugin_home,"trust_policy":$trust_policy,"workspace":$workspace}'
      else
        echo "[kelvin-plugin] smoke ok (${api_key_env} missing path)"
        echo "  plugin:      ${plugin_id}"
        echo "  plugin_home: ${plugin_home}"
        echo "  trust:       ${trust_policy}"
      fi
      return 0
    fi
    cat "${host_output}" >&2
    echo "smoke failed: expected a clear '${api_key_env} is required' message when ${api_key_env} is unset" >&2
    exit 1
  fi

  if [[ "${status}" -ne 0 ]]; then
    cat "${host_output}" >&2
    echo "smoke failed: kelvin-host exited with status ${status}" >&2
    exit 1
  fi

  if [[ "${json_output}" == "1" ]]; then
    jq -cn \
      --arg plugin_id "${plugin_id}" \
      --arg api_key_env "${api_key_env}" \
      --arg plugin_home "${plugin_home}" \
      --arg trust_policy "${trust_policy}" \
      --arg workspace "${workspace}" \
      '{"ok":true,"mode":"live_key_success","plugin_id":$plugin_id,"api_key_env":$api_key_env,"plugin_home":$plugin_home,"trust_policy":$trust_policy,"workspace":$workspace}'
  else
    echo "[kelvin-plugin] smoke ok (${api_key_env} present path)"
    echo "  plugin:      ${plugin_id}"
    echo "  plugin_home: ${plugin_home}"
    echo "  trust:       ${trust_policy}"
  fi
}

main() {
  require_cmd jq
  require_cmd tar
  local command="${1:-}"
  if [[ -z "${command}" ]]; then
    usage
    exit 1
  fi
  shift || true

  case "${command}" in
    list) cmd_list "$@" ;;
    search) cmd_search "$@" ;;
    new) cmd_new "$@" ;;
    test) cmd_test "$@" ;;
    pack) cmd_pack "$@" ;;
    install) cmd_install "$@" ;;
    index-install) cmd_index_install "$@" ;;
    verify) cmd_verify "$@" ;;
    smoke) cmd_smoke "$@" ;;
    -h|--help) usage ;;
    *) echo "Unknown command: ${command}" >&2; usage; exit 1 ;;
  esac
}

main "$@"
