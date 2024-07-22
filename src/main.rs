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
    let mut console = platform.console().unwrap();
    writeln!(console, "DemoOS starting...").unwrap();
    let mut console = console::init(console);
    logger::init(console, LevelFilter::Info).unwrap();
    let mut rtc = platform.rtc().unwrap();
    let mut _gic = platform.gic().unwrap();

    shell::main(&mut console, &mut rtc);

    info!("Powering off.");
    PlatformImpl::power_off();
}
