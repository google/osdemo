#![no_main]
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

mod apps;
mod console;
pub mod drivers;
mod exceptions;
mod logger;
mod platform;

use apps::shell;
use core::fmt::Write;
use log::{info, LevelFilter};
use platform::{Platform, PlatformImpl};

#[no_mangle]
extern "C" fn main() {
    // SAFETY: We only call `PlatformImpl::create` here, once on boot.
    let mut platform = unsafe { PlatformImpl::create() };
    let mut parts = platform.parts().unwrap();
    writeln!(parts.console, "DemoOS starting...").unwrap();
    let mut console = console::init(parts.console);
    logger::init(console, LevelFilter::Info).unwrap();

    info!("Initialising GIC...");
    parts.gic.setup();

    shell::main(&mut console, &mut parts.rtc);

    info!("Powering off.");
    PlatformImpl::power_off();
}
