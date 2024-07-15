#![no_main]
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

mod apps;
mod exceptions;
mod logger;
mod platform;

use apps::shell;
use core::{fmt::Write, panic::PanicInfo};
use log::{error, info, LevelFilter};
use platform::{Platform, PlatformImpl};

#[no_mangle]
extern "C" fn main() {
    // SAFETY: We only call `PlatformImpl::console` here, once on boot.
    let mut console = unsafe { PlatformImpl::console() };
    writeln!(console, "DemoOS starting...").unwrap();
    let mut console = logger::init(console, LevelFilter::Info).unwrap();

    shell::main(&mut console);

    info!("Powering off.");
    PlatformImpl::power_off();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("{info}");
    PlatformImpl::power_off();
}
