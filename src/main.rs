#![no_main]
#![no_std]

mod exceptions;
mod platform;

use core::panic::PanicInfo;
use log::error;
use platform::{Platform, PlatformImpl};

#[no_mangle]
extern "C" fn main() {
    PlatformImpl::power_off();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("{info}");
    loop {}
}
