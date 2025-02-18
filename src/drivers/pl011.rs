// Copyright 2025 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use crate::console::InterruptRead;
use arm_gic::IntId;
use arm_pl011_uart::Uart;
use core::hint::spin_loop;

impl InterruptRead for Uart<'_> {
    fn handle_irq(&mut self, _intid: IntId) {}

    fn wait_for_irq() {
        spin_loop();
    }
}
