//! Kelvin DeepWiki plugin — wasm_tool_v1 ABI.
//!
//! Queries the DeepWiki MCP API (mcp.deepwiki.com) for the KelvinClaw repository.
//! No API key required — the repository is public.
//!
//! Call with:
//!   - topic="..."    → read_wiki_contents  (fetch a specific documentation topic)
//!   - question="..." → ask_question        (ask a natural-language question)
//!   - neither        → read_wiki_structure (get the documentation table of contents)

#![no_std]

#[link(wasm_import_module = "claw")]
extern "C" {
    fn log(level: i32, msg_ptr: i32, msg_len: i32) -> i32;
    fn http_call(req_ptr: i32, req_len: i32, resp_ptr: i32, resp_max: i32) -> i32;
}

// ---------------------------------------------------------------------------
// Arena allocator (2 MiB static heap — DeepWiki responses can be large)
// ---------------------------------------------------------------------------

const HEAP_SIZE: usize = 2 * 1024 * 1024;
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
// Write helpers
// ---------------------------------------------------------------------------

fn write_bytes(buf: &mut [u8], pos: &mut usize, src: &[u8]) -> bool {
    let end = *pos + src.len();
    if end > buf.len() {
        return false;
    }
    buf[*pos..end].copy_from_slice(src);
    *pos = end;
    true
}

fn write_byte(buf: &mut [u8], pos: &mut usize, b: u8) -> bool {
    if *pos >= buf.len() {
        return false;
    }
    buf[*pos] = b;
    *pos += 1;
    true
}

fn write_u32(buf: &mut [u8], pos: &mut usize, mut n: u32) -> bool {
    if n == 0 {
        return write_byte(buf, pos, b'0');
    }
    let mut tmp = [0u8; 10];
    let mut i = 10usize;
    while n > 0 {
        i -= 1;
        tmp[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    write_bytes(buf, pos, &tmp[i..])
}

fn write_json_str(buf: &mut [u8], pos: &mut usize, s: &[u8]) -> bool {
    if !write_byte(buf, pos, b'"') {
        return false;
    }
    for &b in s {
        let ok = match b {
            b'"' => write_bytes(buf, pos, b"\\\""),
            b'\\' => write_bytes(buf, pos, b"\\\\"),
            b'\n' => write_bytes(buf, pos, b"\\n"),
            b'\r' => write_bytes(buf, pos, b"\\r"),
            b'\t' => write_bytes(buf, pos, b"\\t"),
            c if c < 0x20 => {
                let hi = c >> 4;
                let lo = c & 0xf;
                let hc = if hi < 10 { b'0' + hi } else { b'a' + hi - 10 };
                let lc = if lo < 10 { b'0' + lo } else { b'a' + lo - 10 };
                write_bytes(buf, pos, b"\\u00")
                    && write_byte(buf, pos, hc)
                    && write_byte(buf, pos, lc)
            }
            _ => write_byte(buf, pos, b),
        };
        if !ok {
            return false;
        }
    }
    write_byte(buf, pos, b'"')
}

// ---------------------------------------------------------------------------
// JSON parsing helpers (no_std, no serde)
// ---------------------------------------------------------------------------

fn skip_ws(json: &[u8], mut i: usize) -> usize {
    while i < json.len() {
        match json[i] {
            b' ' | b'\t' | b'\n' | b'\r' => i += 1,
            _ => break,
        }
    }
    i
}

fn find_field_value(json: &[u8], field: &[u8]) -> Option<usize> {
    let mut i = 0;
    while i + field.len() + 2 < json.len() {
        if json[i] != b'"' {
            i += 1;
            continue;
        }
        let name_start = i + 1;
        let name_end = name_start + field.len();
        if name_end >= json.len() {
            break;
        }
        if &json[name_start..name_end] == field && json[name_end] == b'"' {
            let mut j = skip_ws(json, name_end + 1);
            if j < json.len() && json[j] == b':' {
                j = skip_ws(json, j + 1);
                return Some(j);
            }
        }
        i += 1;
    }
    None
}

fn extract_str_value(json: &[u8], start: usize) -> Option<&[u8]> {
    if start >= json.len() || json[start] != b'"' {
        return None;
    }
    let s = start + 1;
    let mut i = s;
    while i < json.len() {
        if json[i] == b'\\' {
            i += 2;
            continue;
        }
        if json[i] == b'"' {
            return Some(&json[s..i]);
        }
        i += 1;
    }
    None
}

fn extract_str_field<'a>(json: &'a [u8], field: &[u8]) -> Option<&'a [u8]> {
    extract_str_value(json, find_field_value(json, field)?)
}

fn extract_int_field(json: &[u8], field: &[u8]) -> Option<i64> {
    let mut i = find_field_value(json, field)?;
    if i >= json.len() {
        return None;
    }
    let negative = json[i] == b'-';
    if negative {
        i += 1;
    }
    if i >= json.len() || !json[i].is_ascii_digit() {
        return None;
    }
    let mut n: i64 = 0;
    while i < json.len() && json[i].is_ascii_digit() {
        n = n.saturating_mul(10).saturating_add((json[i] - b'0') as i64);
        i += 1;
    }
    Some(if negative { -n } else { n })
}

// ---------------------------------------------------------------------------
// JSON string unescaping
// ---------------------------------------------------------------------------

fn unescape_json_str(src: &[u8], buf: &mut [u8], pos: &mut usize) -> bool {
    let mut i = 0;
    while i < src.len() {
        if src[i] == b'\\' && i + 1 < src.len() {
            let ch = match src[i + 1] {
                b'"' => b'"',
                b'\\' => b'\\',
                b'/' => b'/',
                b'n' => b'\n',
                b'r' => b'\r',
                b't' => b'\t',
                other => other,
            };
            if !write_byte(buf, pos, ch) {
                return false;
            }
            i += 2;
        } else {
            if !write_byte(buf, pos, src[i]) {
                return false;
            }
            i += 1;
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Logging
// ---------------------------------------------------------------------------

fn log_str(level: i32, msg: &[u8]) {
    let ptr = alloc(msg.len() as i32);
    if ptr == 0 {
        return;
    }
    unsafe {
        core::ptr::copy_nonoverlapping(msg.as_ptr(), ptr as *mut u8, msg.len());
        log(level, ptr, msg.len() as i32);
    }
}

// ---------------------------------------------------------------------------
// Result builders
// ---------------------------------------------------------------------------

fn error_result(msg: &[u8]) -> i64 {
    let buf_size = 64 + msg.len() * 4;
    let ptr = alloc(buf_size as i32);
    if ptr == 0 {
        return 0;
    }
    let buf = unsafe { core::slice::from_raw_parts_mut(ptr as *mut u8, buf_size) };
    let mut pos = 0usize;
    let _ = write_bytes(buf, &mut pos, b"{\"summary\":")
        && write_json_str(buf, &mut pos, msg)
        && write_bytes(buf, &mut pos, b",\"output\":")
        && write_json_str(buf, &mut pos, msg)
        && write_bytes(buf, &mut pos, b",\"visible_text\":null,\"is_error\":true}");
    ((ptr as i64) << 32) | (pos as i64)
}

// ---------------------------------------------------------------------------
// Input validation
// ---------------------------------------------------------------------------

/// Topic / question strings: printable ASCII only, 1–512 characters.
fn validate_arg(arg: &[u8]) -> bool {
    if arg.is_empty() || arg.len() > 512 {
        return false;
    }
    for &b in arg {
        if b < 0x20 || b == 0x7f {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const REPO_NAME: &[u8] = b"AgenticHighway/kelvinclaw";
const MCP_ENDPOINT: &[u8] = b"https://mcp.deepwiki.com/mcp";
const RESP_MAX: i32 = 192 * 1024; // 192 KiB per buffer

// ---------------------------------------------------------------------------
// SSE unwrapping
// ---------------------------------------------------------------------------

/// If `data` looks like an SSE event stream, return the JSON payload from the
/// first `data: {` line.  Otherwise return `data` unchanged (plain JSON path).
///
/// The MCP streamable-HTTP transport may respond with SSE even for single
/// tool-call results.  Each event looks like:
///   data: {"jsonrpc":"2.0","id":1,"result":{...}}\n\n
fn unwrap_sse(data: &[u8]) -> &[u8] {
    const MARKER: &[u8] = b"data: {";
    let mut i = 0;
    while i + MARKER.len() <= data.len() {
        if &data[i..i + MARKER.len()] == MARKER {
            let start = i + MARKER.len() - 1; // include the opening '{'
            let mut end = start + 1;
            while end < data.len() && data[end] != b'\n' && data[end] != b'\r' {
                end += 1;
            }
            return &data[start..end];
        }
        i += 1;
    }
    data // not SSE — return unchanged
}

// ---------------------------------------------------------------------------
// handle_tool_call — main entry point
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn handle_tool_call(ptr: i32, len: i32) -> i64 {
    if len <= 0 {
        return error_result(b"empty input");
    }
    let input = unsafe { core::slice::from_raw_parts(ptr as *const u8, len as usize) };

    let has_topic = extract_str_field(input, b"topic").is_some();
    let has_question = extract_str_field(input, b"question").is_some();

    if has_topic && has_question {
        return error_result(b"provide either 'topic' or 'question', not both");
    }

    // mode 0 = read_wiki_contents (topic)
    // mode 1 = ask_question (question)
    // mode 2 = read_wiki_structure (no args)
    let mode: u8 = if has_topic { 0 } else if has_question { 1 } else { 2 };

    // Unescape and validate the string argument (modes 0 and 1 only)
    let arg_buf_ptr = alloc(768);
    if arg_buf_ptr == 0 {
        return error_result(b"alloc failed");
    }
    let arg_buf = unsafe { core::slice::from_raw_parts_mut(arg_buf_ptr as *mut u8, 768) };
    let mut arg_len = 0usize;

    if mode < 2 {
        let raw = if mode == 0 {
            extract_str_field(input, b"topic").unwrap()
        } else {
            extract_str_field(input, b"question").unwrap()
        };
        if !unescape_json_str(raw, arg_buf, &mut arg_len) {
            return error_result(b"argument too long");
        }
        if !validate_arg(&arg_buf[..arg_len]) {
            return error_result(
                b"invalid argument: must be 1-512 printable ASCII characters",
            );
        }
    }
    let arg = &arg_buf[..arg_len];

    // Build the MCP JSON-RPC body:
    // {"jsonrpc":"2.0","method":"tools/call","params":{"name":"<tool>","arguments":{"repoUrl":"...[,"topic"|"question":"..."]}},"id":1}
    let mcp_body_ptr = alloc(2048);
    if mcp_body_ptr == 0 {
        return error_result(b"alloc failed");
    }
    let mcp_body_buf =
        unsafe { core::slice::from_raw_parts_mut(mcp_body_ptr as *mut u8, 2048) };
    let mut mcp_body_len = 0usize;

    let mcp_tool: &[u8] = match mode {
        0 => b"read_wiki_contents",
        1 => b"ask_question",
        _ => b"read_wiki_structure",
    };

    let ok = write_bytes(
        mcp_body_buf,
        &mut mcp_body_len,
        b"{\"jsonrpc\":\"2.0\",\"method\":\"tools/call\",\"params\":{\"name\":\"",
    ) && write_bytes(mcp_body_buf, &mut mcp_body_len, mcp_tool)
        && write_bytes(
            mcp_body_buf,
            &mut mcp_body_len,
            b"\",\"arguments\":{\"repoName\":",
        )
        && write_json_str(mcp_body_buf, &mut mcp_body_len, REPO_NAME)
        && (mode >= 2
            || (if mode == 0 {
                write_bytes(mcp_body_buf, &mut mcp_body_len, b",\"topic\":")
            } else {
                write_bytes(mcp_body_buf, &mut mcp_body_len, b",\"question\":")
            } && write_json_str(mcp_body_buf, &mut mcp_body_len, arg)))
        && write_bytes(mcp_body_buf, &mut mcp_body_len, b"}},\"id\":1}");

    if !ok {
        return error_result(b"failed to build MCP request body");
    }
    let mcp_body = &mcp_body_buf[..mcp_body_len];

    // Build the outer http_call request JSON:
    // {"url":"...","method":"POST","headers":{"Content-Type":"application/json","Accept":"application/json"},"body":"<mcp_body_json_escaped>"}
    let req_ptr = alloc(4096);
    if req_ptr == 0 {
        return error_result(b"alloc failed");
    }
    let req_buf = unsafe { core::slice::from_raw_parts_mut(req_ptr as *mut u8, 4096) };
    let mut req_len = 0usize;
    let ok = write_bytes(req_buf, &mut req_len, b"{\"url\":")
        && write_json_str(req_buf, &mut req_len, MCP_ENDPOINT)
        && write_bytes(
            req_buf,
            &mut req_len,
            b",\"method\":\"POST\",\"headers\":{\"Content-Type\":\"application/json\",\"Accept\":\"application/json, text/event-stream\"},\"body\":",
        )
        && write_json_str(req_buf, &mut req_len, mcp_body)
        && write_bytes(req_buf, &mut req_len, b"}");

    if !ok {
        return error_result(b"failed to build HTTP request");
    }

    log_str(2, b"kelvin_wiki_fetch: querying DeepWiki MCP");

    // Make the HTTP call
    let resp_ptr = alloc(RESP_MAX);
    if resp_ptr == 0 {
        return error_result(b"alloc failed for response buffer");
    }
    let resp_len = unsafe { http_call(req_ptr, req_len as i32, resp_ptr, RESP_MAX) };
    if resp_len <= 0 {
        return error_result(b"http_call failed");
    }
    let resp = unsafe { core::slice::from_raw_parts(resp_ptr as *const u8, resp_len as usize) };

    // Check HTTP status
    let status = extract_int_field(resp, b"status").unwrap_or(0);
    if status != 200 {
        let err_ptr = alloc(128);
        if err_ptr == 0 {
            return error_result(b"DeepWiki MCP request failed");
        }
        let err_buf = unsafe { core::slice::from_raw_parts_mut(err_ptr as *mut u8, 128) };
        let mut ep = 0usize;
        let ok = write_bytes(err_buf, &mut ep, b"DeepWiki MCP request failed (HTTP ")
            && write_u32(err_buf, &mut ep, status.max(0) as u32)
            && write_byte(err_buf, &mut ep, b')');
        if ok {
            return error_result(&err_buf[..ep]);
        }
        return error_result(b"DeepWiki MCP request failed (non-200 status)");
    }

    // Extract and unescape the HTTP response body to get the raw MCP JSON response
    let body_raw = match extract_str_field(resp, b"body") {
        Some(b) => b,
        None => return error_result(b"missing body in HTTP response"),
    };
    let mcp_resp_ptr = alloc(RESP_MAX);
    if mcp_resp_ptr == 0 {
        return error_result(b"alloc failed for MCP response buffer");
    }
    let mcp_resp_buf =
        unsafe { core::slice::from_raw_parts_mut(mcp_resp_ptr as *mut u8, RESP_MAX as usize) };
    let mut mcp_resp_len = 0usize;
    let _ = unescape_json_str(body_raw, mcp_resp_buf, &mut mcp_resp_len);
    // Unwrap SSE framing if present: extracts JSON from "data: {...}" lines.
    let mcp_resp = unwrap_sse(&mcp_resp_buf[..mcp_resp_len]);

    // Check for a JSON-RPC level error
    if find_field_value(mcp_resp, b"error").is_some() {
        let msg_raw = extract_str_field(mcp_resp, b"message")
            .unwrap_or(b"DeepWiki MCP returned an error");
        let msg_ptr = alloc(512);
        if msg_ptr == 0 {
            return error_result(b"DeepWiki MCP returned an error");
        }
        let msg_buf = unsafe { core::slice::from_raw_parts_mut(msg_ptr as *mut u8, 512) };
        let mut msg_len = 0usize;
        let _ = unescape_json_str(msg_raw, msg_buf, &mut msg_len);
        return error_result(&msg_buf[..msg_len]);
    }

    // Extract the text content from result.content[0].text.
    // In the MCP response JSON the value "text" appears as the value of "type"
    // before appearing as a key ("text":"..."), so find_field_value correctly
    // returns the first "text" KEY, which is the actual content.
    let content_raw = match extract_str_field(mcp_resp, b"text") {
        Some(t) => t,
        None => return error_result(b"no text content in DeepWiki response"),
    };
    let content_ptr = alloc(RESP_MAX);
    if content_ptr == 0 {
        return error_result(b"alloc failed for content buffer");
    }
    let content_buf =
        unsafe { core::slice::from_raw_parts_mut(content_ptr as *mut u8, RESP_MAX as usize) };
    let mut content_len = 0usize;
    let _ = unescape_json_str(content_raw, content_buf, &mut content_len);
    let content = &content_buf[..content_len];

    // Build summary line
    let sum_ptr = alloc(300);
    if sum_ptr == 0 {
        return error_result(b"alloc failed");
    }
    let sum_buf = unsafe { core::slice::from_raw_parts_mut(sum_ptr as *mut u8, 300) };
    let mut sum_len = 0usize;
    let label: &[u8] = match mode {
        0 => b"deepwiki topic: ",
        1 => b"deepwiki question: ",
        _ => b"deepwiki: structure",
    };
    let _ = write_bytes(sum_buf, &mut sum_len, label);
    if mode < 2 {
        let display = if arg.len() > 60 { &arg[..60] } else { arg };
        let _ = write_bytes(sum_buf, &mut sum_len, display);
    }
    let summary = &sum_buf[..sum_len];

    // Build ToolCallResult JSON
    let result_size = 64usize
        .saturating_add(summary.len().saturating_mul(2))
        .saturating_add(content.len().saturating_mul(2));
    let result_ptr = alloc(result_size as i32);
    if result_ptr == 0 {
        return error_result(b"alloc failed for result");
    }
    let result_buf =
        unsafe { core::slice::from_raw_parts_mut(result_ptr as *mut u8, result_size) };
    let mut result_len = 0usize;
    let ok = write_bytes(result_buf, &mut result_len, b"{\"summary\":")
        && write_json_str(result_buf, &mut result_len, summary)
        && write_bytes(result_buf, &mut result_len, b",\"output\":")
        && write_json_str(result_buf, &mut result_len, content)
        && write_bytes(
            result_buf,
            &mut result_len,
            b",\"visible_text\":null,\"is_error\":false}",
        );
    if !ok {
        return error_result(b"failed to build result JSON");
    }

    ((result_ptr as i64) << 32) | (result_len as i64)
}

// v1 ABI backward-compatibility stub
#[no_mangle]
pub extern "C" fn run() -> i32 {
    0
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
