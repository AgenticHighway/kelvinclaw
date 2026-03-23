#![no_std]

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

#[no_mangle]
pub extern "C" fn infer(req_ptr: i32, req_len: i32) -> i64 {
    let input = unsafe {
        let ptr = core::ptr::addr_of!(HEAP).cast::<u8>().add(req_ptr as usize);
        core::slice::from_raw_parts(ptr, req_len as usize)
    };

    let prompt = extract_str_field(input, b"user_prompt").unwrap_or(b"(no prompt)");

    let out = build_output(prompt);

    let out_ptr = alloc(out.len() as i32);
    if out_ptr == 0 { return 0; }
    unsafe {
        let dst = core::ptr::addr_of_mut!(HEAP).cast::<u8>().add(out_ptr as usize);
        core::ptr::copy_nonoverlapping(out.as_ptr(), dst, out.len());
    }
    ((out_ptr as i64) << 32) | (out.len() as i64)
}

fn extract_str_field<'a>(json: &'a [u8], field: &[u8]) -> Option<&'a [u8]> {
    let mut needle = [0u8; 64];
    let mut ni = 0;
    needle[ni] = b'"'; ni += 1;
    for &b in field { needle[ni] = b; ni += 1; }
    needle[ni] = b'"'; ni += 1;
    needle[ni] = b':'; ni += 1;
    needle[ni] = b'"'; ni += 1;
    let needle = &needle[..ni + 1];

    let pos = find_bytes(json, needle)?;
    let start = pos + needle.len();
    let mut i = start;
    while i < json.len() {
        if json[i] == b'\\' { i += 2; continue; }
        if json[i] == b'"' { return Some(&json[start..i]); }
        i += 1;
    }
    None
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() { return Some(0); }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn build_output(prompt_bytes: &[u8]) -> &'static [u8] {
    const TEMPLATE_PRE: &[u8] = b"{\"assistant_text\":\"";
    const TEMPLATE_POST: &[u8] = b"\",\"stop_reason\":\"end_turn\",\"tool_calls\":[],\"usage\":null}";

    let total = TEMPLATE_PRE.len() + prompt_bytes.len() + TEMPLATE_POST.len();
    let ptr = alloc(total as i32) as usize;

    unsafe {
        let base = core::ptr::addr_of_mut!(HEAP).cast::<u8>().add(ptr);
        let mut offset = 0;
        core::ptr::copy_nonoverlapping(TEMPLATE_PRE.as_ptr(), base.add(offset), TEMPLATE_PRE.len());
        offset += TEMPLATE_PRE.len();
        core::ptr::copy_nonoverlapping(prompt_bytes.as_ptr(), base.add(offset), prompt_bytes.len());
        offset += prompt_bytes.len();
        core::ptr::copy_nonoverlapping(TEMPLATE_POST.as_ptr(), base.add(offset), TEMPLATE_POST.len());
        core::slice::from_raw_parts(base, total)
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}