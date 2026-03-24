// Copyright 2026 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use super::InterruptDriven;
use arm_gic::{IntId, InterruptGroup, gicv3::GicCpuInterface};
use uart_16550::{Uart16550, backend::Backend};

impl<B: Backend> InterruptDriven for Uart16550<B> {
    fn handle_irq(&mut self, intid: IntId) {
        GicCpuInterface::end_interrupt(intid, InterruptGroup::Group1);
    }
}
