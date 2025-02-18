// Copyright 2025 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use crate::console::{Console, InterruptRead};
use arm_gic::IntId;
use arm_pl011_uart::Uart;
use embedded_io::{Read, ReadExactError};

impl InterruptRead for Uart<'_> {
    fn handle_irq(&mut self, _intid: IntId) {}

    fn read_char(console: &mut Console<Self>) -> Result<u8, ReadExactError<Self::Error>> {
        let mut c = [0];
        console.read_exact(&mut c)?;
        Ok(c[0])
    }
}
