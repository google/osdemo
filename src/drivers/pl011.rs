// Copyright 2025 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use super::InterruptDriven;
use arm_gic::{
    IntId,
    gicv3::{GicV3, InterruptGroup},
};
use arm_pl011_uart::{Interrupts, Uart};

impl InterruptDriven for Uart<'_> {
    fn handle_irq(&mut self, intid: IntId) {
        self.clear_interrupts(Interrupts::RXI);
        GicV3::end_interrupt(intid, InterruptGroup::Group1);
    }
}
