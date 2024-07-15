#![no_main]
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

mod exceptions;
mod platform;

use core::{fmt::Write, panic::PanicInfo};
use log::error;
use platform::{Platform, PlatformImpl};

#[no_mangle]
extern "C" fn main() {
    // SAFETY: We only call `PlatformImpl::console` here, once on boot.
    let mut console = unsafe { PlatformImpl::console() };
    writeln!(console, "DemoOS starting...").unwrap();

    PlatformImpl::power_off();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("{info}");
    PlatformImpl::power_off();
}
