// Copyright 2024 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use crate::interrupts::handle_irq;
use core::arch::asm;
use log::trace;

#[no_mangle]
extern "C" fn sync_exception_current(elr: u64, _spsr: u64) {
    panic!(
        "Unexpected sync_exception_current, esr={:#x}, far={:#x}, elr={:#x}",
        esr(),
        far(),
        elr
    );
}

#[no_mangle]
extern "C" fn irq_current(_elr: u64, _spsr: u64) {
    trace!("irq_current");
    handle_irq();
}

#[no_mangle]
extern "C" fn fiq_current(_elr: u64, _spsr: u64) {
    panic!("Unexpected fiq_current");
}

#[no_mangle]
extern "C" fn serr_current(_elr: u64, _spsr: u64) {
    panic!("Unexpected serr_current");
}

#[no_mangle]
extern "C" fn sync_lower(_elr: u64, _spsr: u64) {
    panic!("Unexpected sync_lower");
}

#[no_mangle]
extern "C" fn irq_lower(_elr: u64, _spsr: u64) {
    panic!("Unexpected irq_lower");
}

#[no_mangle]
extern "C" fn fiq_lower(_elr: u64, _spsr: u64) {
    panic!("Unexpected fiq_lower");
}

#[no_mangle]
extern "C" fn serr_lower(_elr: u64, _spsr: u64) {
    panic!("Unexpected serr_lower");
}

fn esr() -> u64 {
    let mut esr: u64;
    // SAFETY: This only reads a system register.
    unsafe {
        asm!("mrs {esr}, esr_el1", esr = out(reg) esr);
    }
    esr
}

fn far() -> u64 {
    let mut far: u64;
    // SAFETY: This only reads a system register.
    unsafe {
        asm!("mrs {far}, far_el1", far = out(reg) far);
    }
    far
}
