#![no_std]
#![no_main]

use core::{ffi::c_int, panic::PanicInfo};

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    loop {}
}

#[unsafe(no_mangle)]
pub extern "system" fn _start() -> c_int {
    0
}
