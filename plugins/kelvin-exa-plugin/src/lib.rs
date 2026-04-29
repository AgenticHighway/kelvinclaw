//! Kelvin Exa Search plugin — wasm_tool_v1 ABI.
//!
//! Calls the Exa `/search` API (https://exa.ai) for AI-powered web search.
//! Returns titles, URLs, and the most relevant text snippet for each result
//! (highlight → summary → text fallback chain).
//!
//! Requires `EXA_API_KEY` in the host environment (declared in
//! `capability_scopes.env_allow`).

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

fn extract_bool_field(json: &[u8], field: &[u8]) -> Option<bool> {
    let i = find_field_value(json, field)?;
    if i + 4 <= json.len() && &json[i..i + 4] == b"true" {
        Some(true)
    } else if i + 5 <= json.len() && &json[i..i + 5] == b"false" {
        Some(false)
    } else {
        None
    }
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

fn extract_array_field<'a>(json: &'a [u8], field: &[u8]) -> Option<&'a [u8]> {
    let start = find_field_value(json, field)?;
    let end = find_matching(json, start, b'[', b']')?;
    Some(&json[start..=end])
}

// ---------------------------------------------------------------------------
// Array iterator — walks JSON values inside a `[...]` slice
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

    fn next_string(&mut self) -> Option<&'a [u8]> {
        loop {
            if self.pos >= self.data.len() {
                return None;
            }
            match self.data[self.pos] {
                b' ' | b'\t' | b'\n' | b'\r' | b',' => self.pos += 1,
                b']' => return None,
                b'"' => break,
                _ => {
                    self.pos += 1;
                }
            }
        }
        let s = extract_str_value(self.data, self.pos)?;
        // Advance past the closing quote.
        self.pos += 1 + s.len() + 1;
        Some(s)
    }
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
        && write_bytes(buf, &mut pos, b",\"visible_text\":")
        && write_json_str(buf, &mut pos, msg)
        && write_bytes(buf, &mut pos, b",\"is_error\":true}");
    ((ptr as i64) << 32) | (pos as i64)
}

// ---------------------------------------------------------------------------
// Type validation (strict allowlists, matched against the Exa API surface)
// ---------------------------------------------------------------------------

const VALID_SEARCH_TYPES: &[&[u8]] = &[b"auto", b"neural", b"fast"];
const VALID_LIVECRAWL: &[&[u8]] = &[b"never", b"fallback", b"preferred", b"always"];
const VALID_CATEGORIES: &[&[u8]] = &[
    b"company",
    b"research paper",
    b"news",
    b"personal site",
    b"financial report",
    b"people",
];

fn allowed(value: &[u8], allowlist: &[&[u8]]) -> bool {
    for v in allowlist {
        if value == *v {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// handle_tool_call — main entry point
// ---------------------------------------------------------------------------

const RESP_MAX: i32 = 192 * 1024; // 192 KiB response buffer (Exa responses can include text)

#[no_mangle]
pub extern "C" fn handle_tool_call(ptr: i32, len: i32) -> i64 {
    if len <= 0 {
        return 0;
    }
    let input = unsafe { core::slice::from_raw_parts(ptr as *const u8, len as usize) };

    // --- Required: query ---
    let query_raw = match extract_str_field(input, b"query") {
        Some(q) => q,
        None => return error_result(b"missing query argument"),
    };
    let qbuf_ptr = alloc(2048);
    if qbuf_ptr == 0 {
        return error_result(b"alloc failed");
    }
    let qbuf = unsafe { core::slice::from_raw_parts_mut(qbuf_ptr as *mut u8, 2048) };
    let mut qlen = 0usize;
    if !unescape_json_str(query_raw, qbuf, &mut qlen) {
        return error_result(b"query too long");
    }
    let query = &qbuf[..qlen];

    // --- Optional: num_results (1..=25, default 5) ---
    let num_results = {
        let n = extract_int_field(input, b"num_results").unwrap_or(5);
        n.max(1).min(25) as u32
    };

    // --- Optional: type (validated against allowlist) ---
    let type_value = match extract_str_field(input, b"type") {
        Some(t) if allowed(t, VALID_SEARCH_TYPES) => Some(t),
        Some(_) => return error_result(b"invalid 'type' (use auto, neural, or fast)"),
        None => None,
    };

    // --- Optional: category (validated against allowlist) ---
    let category_value = match extract_str_field(input, b"category") {
        Some(c) if allowed(c, VALID_CATEGORIES) => Some(c),
        Some(_) => return error_result(b"invalid 'category'"),
        None => None,
    };

    // --- Optional: livecrawl (validated against allowlist) ---
    let livecrawl_value = match extract_str_field(input, b"livecrawl") {
        Some(lc) if allowed(lc, VALID_LIVECRAWL) => Some(lc),
        Some(_) => return error_result(b"invalid 'livecrawl'"),
        None => None,
    };

    // --- Optional: domain filters (raw arrays, embedded verbatim) ---
    let include_domains = extract_array_field(input, b"include_domains");
    let exclude_domains = extract_array_field(input, b"exclude_domains");

    // --- Optional: date filters ---
    let start_date = extract_str_field(input, b"start_published_date");
    let end_date = extract_str_field(input, b"end_published_date");

    // --- Optional: summary flag ---
    let want_summary = extract_bool_field(input, b"summary").unwrap_or(false);

    // --- Get EXA_API_KEY ---
    const KEY_NAME: &[u8] = b"EXA_API_KEY";
    let key_ptr = alloc(512);
    if key_ptr == 0 {
        return error_result(b"alloc failed");
    }
    let key_len =
        unsafe { get_env(KEY_NAME.as_ptr() as i32, KEY_NAME.len() as i32, key_ptr, 512) };
    if key_len <= 0 {
        return error_result(b"EXA_API_KEY not set");
    }
    let api_key = unsafe { core::slice::from_raw_parts(key_ptr as *const u8, key_len as usize) };

    // --- Build the inner Exa request body JSON ---
    let body_ptr = alloc(8192);
    if body_ptr == 0 {
        return error_result(b"alloc failed");
    }
    let body_buf = unsafe { core::slice::from_raw_parts_mut(body_ptr as *mut u8, 8192) };
    let mut blen = 0usize;
    let mut ok = write_bytes(body_buf, &mut blen, b"{\"query\":")
        && write_json_str(body_buf, &mut blen, query)
        && write_bytes(body_buf, &mut blen, b",\"numResults\":")
        && write_u32(body_buf, &mut blen, num_results);

    if let Some(t) = type_value {
        ok = ok
            && write_bytes(body_buf, &mut blen, b",\"type\":")
            && write_json_str(body_buf, &mut blen, t);
    }
    if let Some(c) = category_value {
        ok = ok
            && write_bytes(body_buf, &mut blen, b",\"category\":")
            && write_json_str(body_buf, &mut blen, c);
    }
    if let Some(d) = include_domains {
        ok = ok
            && write_bytes(body_buf, &mut blen, b",\"includeDomains\":")
            && write_bytes(body_buf, &mut blen, d);
    }
    if let Some(d) = exclude_domains {
        ok = ok
            && write_bytes(body_buf, &mut blen, b",\"excludeDomains\":")
            && write_bytes(body_buf, &mut blen, d);
    }
    if let Some(d) = start_date {
        ok = ok
            && write_bytes(body_buf, &mut blen, b",\"startPublishedDate\":")
            && write_json_str(body_buf, &mut blen, d);
    }
    if let Some(d) = end_date {
        ok = ok
            && write_bytes(body_buf, &mut blen, b",\"endPublishedDate\":")
            && write_json_str(body_buf, &mut blen, d);
    }

    // contents: highlights + a short text snippet, optionally a summary.
    ok = ok
        && write_bytes(
            body_buf,
            &mut blen,
            b",\"contents\":{\"highlights\":true,\"text\":{\"maxCharacters\":500}",
        );
    if want_summary {
        ok = ok && write_bytes(body_buf, &mut blen, b",\"summary\":{}");
    }
    if let Some(lc) = livecrawl_value {
        ok = ok
            && write_bytes(body_buf, &mut blen, b",\"livecrawl\":")
            && write_json_str(body_buf, &mut blen, lc);
    }
    ok = ok && write_bytes(body_buf, &mut blen, b"}}");

    if !ok {
        return error_result(b"failed to build request body");
    }
    let body = &body_buf[..blen];

    // --- Build the http_call request envelope ---
    // {"url":"https://api.exa.ai/search","method":"POST","headers":{"x-api-key":"...","x-exa-integration":"kelvinclaw","Content-Type":"application/json","Accept":"application/json"},"body":"<escaped JSON>"}
    let req_ptr = alloc(16384);
    if req_ptr == 0 {
        return error_result(b"alloc failed");
    }
    let req_buf = unsafe { core::slice::from_raw_parts_mut(req_ptr as *mut u8, 16384) };
    let mut rlen = 0usize;
    let ok = write_bytes(
        req_buf,
        &mut rlen,
        b"{\"url\":\"https://api.exa.ai/search\",\"method\":\"POST\",\"headers\":{\"x-api-key\":",
    ) && write_json_str(req_buf, &mut rlen, api_key)
        && write_bytes(
            req_buf,
            &mut rlen,
            b",\"x-exa-integration\":\"kelvinclaw\",\"Content-Type\":\"application/json\",\"Accept\":\"application/json\"},\"body\":",
        )
        && write_json_str(req_buf, &mut rlen, body)
        && write_bytes(req_buf, &mut rlen, b"}");
    if !ok {
        return error_result(b"failed to build HTTP request");
    }

    log_str(2, b"kelvin_exa_search: calling Exa /search API");

    // --- HTTP call ---
    let resp_ptr = alloc(RESP_MAX);
    if resp_ptr == 0 {
        return error_result(b"alloc failed for response buffer");
    }
    let resp_len = unsafe { http_call(req_ptr, rlen as i32, resp_ptr, RESP_MAX) };
    if resp_len <= 0 {
        return error_result(b"http_call failed");
    }
    let resp = unsafe { core::slice::from_raw_parts(resp_ptr as *const u8, resp_len as usize) };

    // --- Check HTTP status ---
    let status = extract_int_field(resp, b"status").unwrap_or(0);
    if status != 200 {
        let err_ptr = alloc(64);
        if err_ptr == 0 {
            return error_result(b"non-200 HTTP response");
        }
        let err_buf = unsafe { core::slice::from_raw_parts_mut(err_ptr as *mut u8, 64) };
        let mut ep = 0usize;
        let _ = write_bytes(err_buf, &mut ep, b"Exa API HTTP ")
            && write_u32(err_buf, &mut ep, status.max(0) as u32);
        return error_result(&err_buf[..ep]);
    }

    // --- Unescape the response body string ---
    let body_raw = match extract_str_field(resp, b"body") {
        Some(b) => b,
        None => return error_result(b"missing body in HTTP response"),
    };
    let body_buf2_ptr = alloc(RESP_MAX);
    if body_buf2_ptr == 0 {
        return error_result(b"alloc failed for body buffer");
    }
    let body_buf2 =
        unsafe { core::slice::from_raw_parts_mut(body_buf2_ptr as *mut u8, RESP_MAX as usize) };
    let mut body2_len = 0usize;
    // Ignore truncation — we work with whatever fits.
    let _ = unescape_json_str(body_raw, body_buf2, &mut body2_len);
    let exa_body = &body_buf2[..body2_len];

    // --- Parse Exa response: { "results": [ { title, url, highlights, summary, text } ... ] } ---
    let results_arr = match extract_array_field(exa_body, b"results") {
        Some(a) => a,
        None => return error_result(b"missing 'results' in Exa response"),
    };

    // --- Format results ---
    let out_ptr = alloc(64 * 1024);
    if out_ptr == 0 {
        return error_result(b"alloc failed for output");
    }
    let out_buf = unsafe { core::slice::from_raw_parts_mut(out_ptr as *mut u8, 64 * 1024) };
    let mut out_len = 0usize;

    let mut iter = ArrayIter::new(results_arr);
    let mut n = 0u32;
    while let Some(result) = iter.next_object() {
        if n >= num_results {
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

        // Snippet fallback: highlights[0] -> summary -> text (truncated).
        let mut wrote_snippet = false;
        if let Some(h_arr) = extract_array_field(result, b"highlights") {
            let mut h_iter = ArrayIter::new(h_arr);
            if let Some(first) = h_iter.next_string() {
                if !first.is_empty() {
                    let _ = write_bytes(out_buf, &mut out_len, b"   ")
                        && unescape_json_str(first, out_buf, &mut out_len)
                        && write_byte(out_buf, &mut out_len, b'\n');
                    wrote_snippet = true;
                }
            }
        }
        if !wrote_snippet {
            if let Some(raw) = extract_str_field(result, b"summary") {
                if !raw.is_empty() {
                    let _ = write_bytes(out_buf, &mut out_len, b"   ")
                        && unescape_json_str(raw, out_buf, &mut out_len)
                        && write_byte(out_buf, &mut out_len, b'\n');
                    wrote_snippet = true;
                }
            }
        }
        if !wrote_snippet {
            if let Some(raw) = extract_str_field(result, b"text") {
                // Cap text snippet to a reasonable length so output stays compact.
                let cap = 300usize.min(raw.len());
                let truncated = &raw[..cap];
                if !truncated.is_empty() {
                    let _ = write_bytes(out_buf, &mut out_len, b"   ")
                        && unescape_json_str(truncated, out_buf, &mut out_len);
                    if cap < raw.len() {
                        let _ = write_bytes(out_buf, &mut out_len, b"...");
                    }
                    let _ = write_byte(out_buf, &mut out_len, b'\n');
                }
            }
        }

        let _ = write_byte(out_buf, &mut out_len, b'\n');
    }

    if n == 0 {
        let _ = write_bytes(out_buf, &mut out_len, b"No results found.");
    }
    let output_text = &out_buf[..out_len];

    // --- Build summary line ---
    let sum_ptr = alloc(128);
    if sum_ptr == 0 {
        return error_result(b"alloc failed");
    }
    let sum_buf = unsafe { core::slice::from_raw_parts_mut(sum_ptr as *mut u8, 128) };
    let mut sum_len = 0usize;
    let q_display = if query.len() > 60 { &query[..60] } else { query };
    let _ = write_bytes(sum_buf, &mut sum_len, b"exa search: ")
        && write_bytes(sum_buf, &mut sum_len, q_display)
        && write_bytes(sum_buf, &mut sum_len, b" (")
        && write_u32(sum_buf, &mut sum_len, n)
        && write_bytes(sum_buf, &mut sum_len, b" results)");
    let summary = &sum_buf[..sum_len];

    // --- Build ToolCallResult JSON ---
    let result_size = 64usize
        .saturating_add(summary.len().saturating_mul(2))
        .saturating_add(output_text.len().saturating_mul(4));
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
