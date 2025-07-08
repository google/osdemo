// Copyright 2024 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

pub mod pl011;
pub mod uart8250;

use arm_gic::{IntId, wfi};

/// Trait for device drivers which can handle interrupts.
pub trait InterruptDriven {
    /// Waits for an IRQ. May return early.
    fn wait_for_irq() {
        wfi();
    }

    /// Handles the given interrupt for the device.
    ///
    /// Note that this may be called with the console locked, so must not try to log anything.
    fn handle_irq(&mut self, intid: IntId);
}
