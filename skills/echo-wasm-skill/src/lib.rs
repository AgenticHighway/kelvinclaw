#![no_std]

#[link(wasm_import_module = "claw")]
extern "C" {
    fn send_message(message_code: i32) -> i32;
}

#[no_mangle]
pub extern "C" fn run() -> i32 {
    // SAFETY: `send_message` is a host import supplied by the trusted Kelvin executive.
    unsafe {
        send_message(42);
    }
    0
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
