#![no_main]
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

mod apps;
mod console;
pub mod drivers;
mod exceptions;
mod logger;
mod pagetable;
mod platform;

use aarch64_paging::paging::PAGE_SIZE;
use apps::shell;
use buddy_system_allocator::Heap;
use core::fmt::Write;
use log::{info, LevelFilter};
use pagetable::IdMap;
use platform::{Platform, PlatformImpl};

const PAGE_HEAP_SIZE: usize = 8 * PAGE_SIZE;
static mut PAGE_HEAP: [u8; PAGE_HEAP_SIZE] = [0; PAGE_HEAP_SIZE];

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

    info!("Initialising page table...");
    let mut page_allocator = Heap::new();
    // SAFETY: We only do this once, as `Once::call_once` guarantees. Nothing else accesses the
    // `HEAP` mutable static.
    unsafe {
        page_allocator.init(PAGE_HEAP.as_mut_ptr() as usize, PAGE_HEAP.len());
    }
    let mut idmap = IdMap::new(page_allocator);
    info!("Mapping platform pages...");
    platform.map_pages(&mut idmap).unwrap();
    info!("Activating page table...");
    unsafe {
        idmap.activate();
    }

    shell::main(&mut console, &mut parts.rtc, &mut parts.gic);

    info!("Powering off.");
    PlatformImpl::power_off();
}
