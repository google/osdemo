#![no_main]
#![no_std]

mod exceptions;

use core::panic::PanicInfo;
use log::error;
use smccc::{psci::system_off, Hvc};

#[no_mangle]
extern "C" fn main() {
    system_off::<Hvc>().unwrap();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("{info}");
    loop {}
}
