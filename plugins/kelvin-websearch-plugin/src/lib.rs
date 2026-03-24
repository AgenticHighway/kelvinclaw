//! Kelvin Web Search plugin — wasm_tool_v1 ABI.
//!
//! Calls the Brave Search API to answer web search queries.
//! Requires `BRAVE_API_KEY` in the host environment (declared in capability_scopes.env_allow).

#![no_std]

#[link(wasm_import_module = "claw")]
extern "C" {
    fn log(level: i32, msg_ptr: i32, msg_len: i32) -> i32;
    fn http_call(req_ptr: i32, req_len: i32, resp_ptr: i32, resp_max: i32) -> i32;
    fn get_env(key_ptr: i32, key_len: i32, val_ptr: i32, val_max: i32) -> i32;
}

// ---------------------------------------------------------------------------
// Arena allocator (1 MiB static heap, bump pointer, no-op dealloc)
// ---------------------------------------------------------------------------

const HEAP_SIZE: usize = 1024 * 1024;
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

/// Write a JSON-escaped string literal (including surrounding quotes).
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

// ---------------------------------------------------------------------------
// JSON parsing helpers (no_std, no serde)
// ---------------------------------------------------------------------------

/// Skip over JSON whitespace.
fn skip_ws(json: &[u8], mut i: usize) -> usize {
    while i < json.len() {
        match json[i] {
            b' ' | b'\t' | b'\n' | b'\r' => i += 1,
            _ => break,
        }
    }
    i
}

/// Find the byte offset of the value for a given JSON object key.
/// Searches for `"<field>"` followed by optional whitespace and `:`.
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

/// Extract a raw (still-escaped) string value starting at `start` (which must be `"`).
/// Returns the bytes between the opening and closing quotes.
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

/// Find the matching closing delimiter for `open` starting at `start`.
fn find_matching(json: &[u8], start: usize, open: u8, close: u8) -> Option<usize> {
    if start >= json.len() || json[start] != open {
        return None;
    }
    let mut depth = 0usize;
    let mut i = start;
    while i < json.len() {
        match json[i] {
            b'"' => {
                i += 1;
                while i < json.len() {
                    if json[i] == b'\\' {
                        i += 2;
                        continue;
                    }
                    if json[i] == b'"' {
                        break;
                    }
                    i += 1;
                }
            }
            c if c == open => depth += 1,
            c if c == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn extract_object_field<'a>(json: &'a [u8], field: &[u8]) -> Option<&'a [u8]> {
    let start = find_field_value(json, field)?;
    let end = find_matching(json, start, b'{', b'}')?;
    Some(&json[start..=end])
}

fn extract_array_field<'a>(json: &'a [u8], field: &[u8]) -> Option<&'a [u8]> {
    let start = find_field_value(json, field)?;
    let end = find_matching(json, start, b'[', b']')?;
    Some(&json[start..=end])
}

// ---------------------------------------------------------------------------
// Array iterator — walks JSON objects inside a `[...]` slice
// ---------------------------------------------------------------------------

struct ArrayIter<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> ArrayIter<'a> {
    fn new(array: &'a [u8]) -> Self {
        let pos = if !array.is_empty() && array[0] == b'[' { 1 } else { 0 };
        ArrayIter { data: array, pos }
    }

    fn next_object(&mut self) -> Option<&'a [u8]> {
        loop {
            if self.pos >= self.data.len() {
                return None;
            }
            match self.data[self.pos] {
                b' ' | b'\t' | b'\n' | b'\r' | b',' => self.pos += 1,
                b']' => return None,
                b'{' => break,
                _ => {
                    self.pos += 1;
                }
            }
        }
        let end = find_matching(self.data, self.pos, b'{', b'}')?;
        let obj = &self.data[self.pos..=end];
        self.pos = end + 1;
        Some(obj)
    }
}

// ---------------------------------------------------------------------------
// URL encoding
// ---------------------------------------------------------------------------

fn url_encode(src: &[u8], buf: &mut [u8], pos: &mut usize) -> bool {
    for &b in src {
        let ok = if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            write_byte(buf, pos, b)
        } else if b == b' ' {
            write_byte(buf, pos, b'+')
        } else {
            let hi = b >> 4;
            let lo = b & 0xf;
            let hc = if hi < 10 { b'0' + hi } else { b'A' + hi - 10 };
            let lc = if lo < 10 { b'0' + lo } else { b'A' + lo - 10 };
            write_byte(buf, pos, b'%') && write_byte(buf, pos, hc) && write_byte(buf, pos, lc)
        };
        if !ok {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------------
// JSON string unescaping — writes decoded bytes into a buffer
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
// Error result builder
// ---------------------------------------------------------------------------

fn error_result(msg: &[u8]) -> i64 {
    let buf_size = 64 + msg.len() * 4; // enough for two JSON-escaped copies
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
        && write_bytes(buf, &mut pos, b",\"visible_text\":")
        && write_json_str(buf, &mut pos, msg)
        && write_bytes(buf, &mut pos, b",\"is_error\":true}");
    ((ptr as i64) << 32) | (pos as i64)
}

// ---------------------------------------------------------------------------
// handle_tool_call — main entry point
// ---------------------------------------------------------------------------

const RESP_MAX: i32 = 128 * 1024; // 128 KiB response buffer

#[no_mangle]
pub extern "C" fn handle_tool_call(ptr: i32, len: i32) -> i64 {
    if len <= 0 {
        return 0;
    }
    let input = unsafe { core::slice::from_raw_parts(ptr as *const u8, len as usize) };

    // The host passes the tool arguments object directly as the input JSON,
    // e.g. {"query":"...","count":5}
    let query_raw = match extract_str_field(input, b"query") {
        Some(q) => q,
        None => return error_result(b"missing query argument"),
    };

    // Unescape the query string
    let qbuf_ptr = alloc(1024);
    if qbuf_ptr == 0 {
        return error_result(b"alloc failed");
    }
    let qbuf = unsafe { core::slice::from_raw_parts_mut(qbuf_ptr as *mut u8, 1024) };
    let mut qlen = 0usize;
    if !unescape_json_str(query_raw, qbuf, &mut qlen) {
        return error_result(b"query too long");
    }
    let query = &qbuf[..qlen];

    let count = {
        let n = extract_int_field(input, b"count").unwrap_or(5);
        n.max(1).min(20) as u32
    };

    // --- Get BRAVE_API_KEY ---
    const KEY_NAME: &[u8] = b"BRAVE_API_KEY";
    let key_ptr = alloc(512);
    if key_ptr == 0 {
        return error_result(b"alloc failed");
    }
    let key_len =
        unsafe { get_env(KEY_NAME.as_ptr() as i32, KEY_NAME.len() as i32, key_ptr, 512) };
    if key_len <= 0 {
        return error_result(b"BRAVE_API_KEY not set");
    }
    let api_key = unsafe { core::slice::from_raw_parts(key_ptr as *const u8, key_len as usize) };

    // --- Build URL ---
    // https://api.search.brave.com/res/v1/web/search?q=<encoded>&count=<n>
    let url_ptr = alloc(4096);
    if url_ptr == 0 {
        return error_result(b"alloc failed");
    }
    let url_buf = unsafe { core::slice::from_raw_parts_mut(url_ptr as *mut u8, 4096) };
    let mut url_len = 0usize;
    let ok = write_bytes(
        url_buf,
        &mut url_len,
        b"https://api.search.brave.com/res/v1/web/search?q=",
    ) && url_encode(query, url_buf, &mut url_len)
        && write_bytes(url_buf, &mut url_len, b"&count=")
        && write_u32(url_buf, &mut url_len, count);
    if !ok {
        return error_result(b"failed to build URL");
    }
    let url = &url_buf[..url_len];

    // --- Build request JSON ---
    // {"url":"...","method":"GET","headers":{"X-Subscription-Token":"<key>","Accept":"application/json"},"body":""}
    let req_ptr = alloc(8192);
    if req_ptr == 0 {
        return error_result(b"alloc failed");
    }
    let req_buf = unsafe { core::slice::from_raw_parts_mut(req_ptr as *mut u8, 8192) };
    let mut req_len = 0usize;
    let ok = write_bytes(req_buf, &mut req_len, b"{\"url\":")
        && write_json_str(req_buf, &mut req_len, url)
        && write_bytes(
            req_buf,
            &mut req_len,
            b",\"method\":\"GET\",\"headers\":{\"X-Subscription-Token\":",
        )
        && write_json_str(req_buf, &mut req_len, api_key)
        && write_bytes(
            req_buf,
            &mut req_len,
            b",\"Accept\":\"application/json\"},\"body\":\"\"}",
        );
    if !ok {
        return error_result(b"failed to build request");
    }

    log_str(2, b"kelvin_websearch: calling Brave Search API");

    // --- HTTP call ---
    let resp_ptr = alloc(RESP_MAX);
    if resp_ptr == 0 {
        return error_result(b"alloc failed for response buffer");
    }
    let resp_len = unsafe { http_call(req_ptr, req_len as i32, resp_ptr, RESP_MAX) };
    if resp_len <= 0 {
        return error_result(b"http_call failed");
    }
    let resp = unsafe { core::slice::from_raw_parts(resp_ptr as *const u8, resp_len as usize) };

    // --- Check HTTP status ---
    let status = extract_int_field(resp, b"status").unwrap_or(0);
    if status != 200 {
        // Try to surface the error body
        let err_ptr = alloc(64);
        if err_ptr == 0 {
            return error_result(b"non-200 HTTP response");
        }
        let err_buf = unsafe { core::slice::from_raw_parts_mut(err_ptr as *mut u8, 64) };
        let mut ep = 0usize;
        let _ = write_bytes(err_buf, &mut ep, b"HTTP ")
            && write_u32(err_buf, &mut ep, status.max(0) as u32);
        return error_result(&err_buf[..ep]);
    }

    // --- Unescape the response body ---
    // resp is {"status":200,"body":"<JSON-escaped Brave response>"}
    let body_raw = match extract_str_field(resp, b"body") {
        Some(b) => b,
        None => return error_result(b"missing body in HTTP response"),
    };

    let body_ptr = alloc(RESP_MAX);
    if body_ptr == 0 {
        return error_result(b"alloc failed for body buffer");
    }
    let body_buf =
        unsafe { core::slice::from_raw_parts_mut(body_ptr as *mut u8, RESP_MAX as usize) };
    let mut body_len = 0usize;
    // Ignore truncation — we work with whatever fits
    let _ = unescape_json_str(body_raw, body_buf, &mut body_len);
    let body = &body_buf[..body_len];

    // --- Parse Brave response ---
    // {"web":{"results":[{"title":"...","url":"...","description":"..."},...]}}
    let web = match extract_object_field(body, b"web") {
        Some(w) => w,
        None => return error_result(b"missing 'web' field in Brave response"),
    };
    let results_arr = match extract_array_field(web, b"results") {
        Some(a) => a,
        None => return error_result(b"missing 'results' in Brave response"),
    };

    // --- Format results ---
    let out_ptr = alloc(32768);
    if out_ptr == 0 {
        return error_result(b"alloc failed for output");
    }
    let out_buf = unsafe { core::slice::from_raw_parts_mut(out_ptr as *mut u8, 32768) };
    let mut out_len = 0usize;

    let mut iter = ArrayIter::new(results_arr);
    let mut n = 0u32;
    while let Some(result) = iter.next_object() {
        if n >= count {
            break;
        }
        n += 1;

        let _ = write_u32(out_buf, &mut out_len, n) && write_bytes(out_buf, &mut out_len, b". ");

        if let Some(raw) = extract_str_field(result, b"title") {
            let _ = unescape_json_str(raw, out_buf, &mut out_len);
        }
        let _ = write_byte(out_buf, &mut out_len, b'\n');

        let _ = write_bytes(out_buf, &mut out_len, b"   ");
        if let Some(raw) = extract_str_field(result, b"url") {
            let _ = unescape_json_str(raw, out_buf, &mut out_len);
        }
        let _ = write_byte(out_buf, &mut out_len, b'\n');

        if let Some(raw) = extract_str_field(result, b"description") {
            let _ = write_bytes(out_buf, &mut out_len, b"   ")
                && unescape_json_str(raw, out_buf, &mut out_len)
                && write_byte(out_buf, &mut out_len, b'\n');
        }

        let _ = write_byte(out_buf, &mut out_len, b'\n');
    }

    if n == 0 {
        let _ = write_bytes(out_buf, &mut out_len, b"No results found.");
    }
    let output_text = &out_buf[..out_len];

    // --- Build summary ---
    let sum_ptr = alloc(128);
    if sum_ptr == 0 {
        return error_result(b"alloc failed");
    }
    let sum_buf = unsafe { core::slice::from_raw_parts_mut(sum_ptr as *mut u8, 128) };
    let mut sum_len = 0usize;
    let q_display = if query.len() > 60 { &query[..60] } else { query };
    let _ = write_bytes(sum_buf, &mut sum_len, b"web search: ")
        && write_bytes(sum_buf, &mut sum_len, q_display)
        && write_bytes(sum_buf, &mut sum_len, b" (")
        && write_u32(sum_buf, &mut sum_len, n)
        && write_bytes(sum_buf, &mut sum_len, b" results)");
    let summary = &sum_buf[..sum_len];

    // --- Build ToolCallResult JSON ---
    // {"summary":"...","output":"...","visible_text":"...","is_error":false}
    let result_size = 64usize
        .saturating_add(summary.len().saturating_mul(2))
        .saturating_add(output_text.len().saturating_mul(4)); // x2 output + x2 visible_text
    let result_ptr = alloc(result_size as i32);
    if result_ptr == 0 {
        return error_result(b"alloc failed for result");
    }
    let result_buf = unsafe { core::slice::from_raw_parts_mut(result_ptr as *mut u8, result_size) };
    let mut result_len = 0usize;
    let ok = write_bytes(result_buf, &mut result_len, b"{\"summary\":")
        && write_json_str(result_buf, &mut result_len, summary)
        && write_bytes(result_buf, &mut result_len, b",\"output\":")
        && write_json_str(result_buf, &mut result_len, output_text)
        && write_bytes(result_buf, &mut result_len, b",\"visible_text\":")
        && write_json_str(result_buf, &mut result_len, output_text)
        && write_bytes(result_buf, &mut result_len, b",\"is_error\":false}");
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
