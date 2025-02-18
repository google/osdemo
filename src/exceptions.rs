// Copyright 2024 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use alloc::collections::btree_map::BTreeMap;
use arm_gic::{gicv3::GicV3, IntId};
use core::arch::asm;
use log::trace;
use percore::{exception_free, ExceptionLock};
use spin::mutex::SpinMutex;

type IrqHandler = &'static (dyn Fn(IntId) + Sync);

static IRQ_HANDLER: ExceptionLock<SpinMutex<BTreeMap<IntId, IrqHandler>>> =
    ExceptionLock::new(SpinMutex::new(BTreeMap::new()));

/// Sets the IRQ handler for the given interrupt ID to the given function.
///
/// Returns the handler that was previously set, if any.
pub fn set_irq_handler(intid: IntId, handler: IrqHandler) -> Option<IrqHandler> {
    trace!("Setting IRQ handler for {:?}", intid);
    exception_free(|token| IRQ_HANDLER.borrow(token).lock().insert(intid, handler))
}

/// Removes the IRQ handler for the given interrupt ID.
///
/// Returns the handler that was previously set, if any.
pub fn remove_irq_handler(intid: IntId) -> Option<IrqHandler> {
    trace!("Removing IRQ handler for {:?}", intid);
    exception_free(|token| IRQ_HANDLER.borrow(token).lock().remove(&intid))
}

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
    let intid = GicV3::get_and_acknowledge_interrupt().expect("No pending interrupt");
    trace!("IRQ: {:?}", intid);
    exception_free(|token| {
        if let Some(handler) = IRQ_HANDLER.borrow(token).lock().get(&intid) {
            handler(intid);
        } else {
            panic!("Unexpected IRQ {:?} with no handler", intid);
        }
    });
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
