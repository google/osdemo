// Copyright 2025 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use crate::platform::{Platform, PlatformImpl};
use alloc::collections::btree_map::BTreeMap;
use arm_gic::{gicv3::GicV3, IntId};
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

/// Asks the GIC what interrupt is pending and then calls the appropriate handler.
///
/// This should be called when there is an irq_current exception.
///
/// Panics if there is no no pending interrupt, or no registered handler for the pending interrupt.
pub fn handle_irq() {
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

/// Performs basic GIC initialisation on boot, ready to start handling interrupts.
pub fn init_gic(gic: &mut GicV3) {
    gic.setup(0);
    PlatformImpl::setup_gic(gic);
}
