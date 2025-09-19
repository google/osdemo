// Copyright 2024 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use super::{Platform, PlatformParts};
use crate::{
    console::Console,
    interrupts::set_shared_irq_handler,
    pagetable::{DEVICE_ATTRIBUTES, MEMORY_ATTRIBUTES},
};
use aarch64_rt::InitialPagetable;
use arm_gic::{IntId, Trigger, gicv3::GicV3};
use arm_pl011_uart::{Interrupts, PL011Registers, Uart, UniqueMmioPointer};
use arm_pl031::Rtc;
use core::ptr::NonNull;

/// Base address of the first PL011 UART.
const UART_BASE_ADDRESS: *mut PL011Registers = 0x900_0000 as _;

/// Base address of the PL031 RTC.
const PL031_BASE_ADDRESS: *mut u32 = 0x901_0000 as _;

/// The QEMU aarch64 virt platform.
pub struct Qemu {
    parts: Option<PlatformParts<Uart<'static>, Rtc>>,
}

impl Qemu {
    const CONSOLE_IRQ: IntId = IntId::spi(1);

    /// Returns the initial hard-coded page table to use before the Rust code starts.
    pub const fn initial_idmap() -> InitialPagetable {
        let mut idmap = [0; 512];
        idmap[0] = DEVICE_ATTRIBUTES.bits();
        idmap[1] = MEMORY_ATTRIBUTES.bits() | 0x40000000;
        idmap[256] = DEVICE_ATTRIBUTES.bits() | 0x4000000000;
        InitialPagetable(idmap)
    }
}

impl Platform for Qemu {
    type Console = Uart<'static>;
    type Rtc = Rtc;

    const RTC_IRQ: IntId = IntId::spi(2);

    unsafe fn create() -> Self {
        let mut uart = Uart::new(
            // SAFETY: UART_BASE_ADDRESS is valid and mapped, and `create` is only called once so
            // there are no aliases
            unsafe { UniqueMmioPointer::new(NonNull::new(UART_BASE_ADDRESS).unwrap()) },
        );
        uart.set_interrupt_masks(Interrupts::RXI);
        Self {
            // SAFETY: The various base addresses are valid and mapped, and `create` is only called
            // once so there are no aliases.
            parts: Some(unsafe {
                PlatformParts {
                    console: uart,
                    rtc: Rtc::new(PL031_BASE_ADDRESS),
                }
            }),
        }
    }

    fn parts(&mut self) -> Option<PlatformParts<Uart<'static>, Rtc>> {
        self.parts.take()
    }

    fn setup_gic(gic: &mut GicV3) {
        gic.set_interrupt_priority(Self::CONSOLE_IRQ, None, 0x10);
        gic.set_trigger(Self::CONSOLE_IRQ, None, Trigger::Level);
        gic.enable_interrupt(Self::CONSOLE_IRQ, None, true);
        set_shared_irq_handler(Self::CONSOLE_IRQ, &Console::<Uart>::handle_irq);
    }
}
