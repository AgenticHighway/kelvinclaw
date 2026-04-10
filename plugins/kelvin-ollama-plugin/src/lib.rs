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
