// Copyright 2024 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use crate::interrupts::handle_irq;
use aarch64_rt::{ExceptionHandlers, RegisterStateRef, exception_handlers};
use arm_sysregs::{
    HcrEl2, read_currentel, read_esr_el1, read_esr_el2, read_far_el1, read_far_el2, read_hcr_el2,
    write_hcr_el2,
};
use log::trace;

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
    if current_el() == 2 {
        read_esr_el2().bits()
    } else {
        read_esr_el1().bits()
    }
}

fn far() -> u64 {
    if current_el() == 2 {
        read_far_el2().bits()
    } else {
        read_far_el1().bits()
    }
}

/// Returns the current exception level.
pub fn current_el() -> u8 {
    read_currentel().el()
}

/// Makes sure Physical IRQs are routed to the current exception level.
pub fn init_irq_routing() {
    if current_el() == 2 {
        // SAFETY: We only set the IMO bit, which is safe.
        unsafe {
            // Route Physical IRQs to EL2.
            write_hcr_el2(read_hcr_el2() | HcrEl2::IMO);
        }
    }
}
