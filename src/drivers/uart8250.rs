// Copyright 2024 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

//! Minimal driver for an 8250 UART. This only implements enough to work with the emulated 8250
//! provided by crosvm, and won't work with real hardware.

use crate::console::InterruptRead;
use arm_gic::{
    IntId,
    gicv3::{GicV3, InterruptGroup},
    wfi,
};
use core::convert::Infallible;
use core::fmt;
use embedded_io::{ErrorType, Read, ReadReady, Write, WriteReady};

/// Minimal driver for an 8250 UART. This only implements enough to work with the emulated 8250
/// provided by crosvm, and won't work with real hardware.
pub struct Uart {
    base_address: *mut u8,
}

impl Uart {
    /// Constructs a new instance of the UART driver for a device at the given base address.
    ///
    /// # Safety
    ///
    /// The given base address must point to the 8 MMIO control registers of an appropriate UART
    /// device, which must be mapped into the address space of the process as device memory and not
    /// have any other aliases.
    pub unsafe fn new(base_address: *mut u8) -> Self {
        Self { base_address }
    }

    /// Writes a single byte to the UART.
    pub fn write_byte(&mut self, byte: u8) {
        // SAFETY: We were promised when `new` was called that the base address points to the
        // control registers of a UART device which is appropriately mapped and not aliased.
        unsafe {
            self.base_address.write_volatile(byte);
        }
    }

    pub fn lsr(&self) -> u8 {
        // SAFETY: We were promised when `new` was called that the base address points to the
        // control registers of a UART device which is appropriately mapped and not aliased.
        unsafe { self.base_address.add(5).read_volatile() }
    }

    /// Returns whether there is data waiting to be read.
    pub fn data_ready(&self) -> bool {
        self.lsr() & 0x01 != 0
    }

    pub fn transmitter_holding_register_empty(&self) -> bool {
        self.lsr() & 0x20 != 0
    }

    /// Reads a single byte from the UART if one is available, or returns None if no data is
    /// currently available to read.
    pub fn read_byte(&mut self) -> Option<u8> {
        if self.data_ready() {
            // SAFETY: We were promised when `new` was called that the base address points to the
            // control registers of a UART device which is appropriately mapped and not aliased.
            Some(unsafe { self.base_address.read_volatile() })
        } else {
            None
        }
    }

    /// Enables the given interrupts.
    pub fn enable_interrupts(&mut self, interrupts: u8) {
        // SAFETY: We were promised when `new` was called that the base address points to the
        // control registers of a UART device which is appropriately mapped and not aliased.
        unsafe {
            self.base_address.add(1).write_volatile(interrupts);
        }
    }
}

impl fmt::Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.as_bytes() {
            self.write_byte(*c);
        }
        Ok(())
    }
}

// SAFETY: `Uart` just contains a pointer to device memory, which can be accessed from any context.
unsafe impl Send for Uart {}

impl ErrorType for Uart {
    type Error = Infallible;
}

impl Write for Uart {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            Ok(0)
        } else {
            self.write_byte(buf[0]);
            Ok(1)
        }
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl WriteReady for Uart {
    fn write_ready(&mut self) -> Result<bool, Self::Error> {
        Ok(self.transmitter_holding_register_empty())
    }
}

impl Read for Uart {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Ok(0);
        }

        loop {
            if let Some(byte) = self.read_byte() {
                buf[0] = byte;
                return Ok(1);
            }
        }
    }
}

impl ReadReady for Uart {
    fn read_ready(&mut self) -> Result<bool, Self::Error> {
        Ok(self.data_ready())
    }
}

impl InterruptRead for Uart {
    fn handle_irq(&mut self, intid: IntId) {
        GicV3::end_interrupt(intid, InterruptGroup::Group1);
    }

    fn wait_for_irq() {
        wfi();
    }
}
