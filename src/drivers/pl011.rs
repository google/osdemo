// Copyright 2025 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use crate::console::InterruptRead;
use arm_gic::{gicv3::GicV3, wfi, IntId};
use arm_pl011_uart::{Interrupts, Uart};

impl InterruptRead for Uart<'_> {
    fn handle_irq(&mut self, intid: IntId) {
        self.clear_interrupts(Interrupts::RXI);
        GicV3::end_interrupt(intid);
    }

    fn wait_for_irq() {
        wfi();
    }
}
