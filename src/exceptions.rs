// Copyright 2024 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use crate::interrupts::handle_irq;
use aarch64_rt::{ExceptionHandlers, RegisterStateRef, exception_handlers};
use core::arch::asm;
use log::trace;

const HCR_EL2_IMO: u64 = 1 << 4;

exception_handlers!(Exceptions);

pub struct Exceptions;

impl ExceptionHandlers for Exceptions {
    extern "C" fn sync_current(register_state: RegisterStateRef) {
        panic!(
            "Unexpected sync_exception_current, esr={:#x}, far={:#x}; saved register state: {register_state:#018x?}",
            esr(),
            far(),
        );
    }

    extern "C" fn irq_current(register_state: RegisterStateRef) {
        trace!("irq_current, register_state: {register_state:#018x?}");
        handle_irq();
    }
}

fn esr() -> u64 {
    let esr: u64;
    if current_el() == 2 {
        // SAFETY: This only reads a system register.
        unsafe {
            asm!("mrs {esr}, esr_el2", esr = out(reg) esr);
        }
    } else {
        // SAFETY: This only reads a system register.
        unsafe {
            asm!("mrs {esr}, esr_el1", esr = out(reg) esr);
        }
    }
    esr
}

fn far() -> u64 {
    let far: u64;
    if current_el() == 2 {
        // SAFETY: This only reads a system register.
        unsafe {
            asm!("mrs {far}, far_el2", far = out(reg) far);
        }
    } else {
        // SAFETY: This only reads a system register.
        unsafe {
            asm!("mrs {far}, far_el1", far = out(reg) far);
        }
    }
    far
}

/// Returns the current exception level.
pub fn current_el() -> u8 {
    let current_el: u64;
    // SAFETY: This only reads a system register.
    unsafe {
        asm!(
            "mrs {current_el}, CurrentEL",
            current_el = out(reg) current_el,
        );
    }
    ((current_el >> 2) & 0b11) as u8
}

fn hcr_el2() -> u64 {
    let value;
    // SAFETY: This only reads a system register.
    unsafe {
        asm!("mrs {value}, hcr_el2", value = out(reg) value);
    }
    value
}

fn write_hcr_el2(value: u64) {
    // SAFETY: Writing to hcr_el2 is safe.
    unsafe {
        asm!("msr hcr_el2, {value}", value = in(reg) value);
    }
}

/// Makes sure Physical IRQs are routed to the current exception level.
pub fn init_irq_routing() {
    if current_el() == 2 {
        // Route Physical IRQs to EL2.
        write_hcr_el2(hcr_el2() | HCR_EL2_IMO);
    }
}
