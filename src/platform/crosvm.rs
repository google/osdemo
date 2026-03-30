// Copyright 2024 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use super::{Platform, PlatformParts};
use crate::{
    console::Console,
    interrupts::set_shared_irq_handler,
    pagetable::{EL1_DEVICE_ATTRIBUTES, EL1_MEMORY_ATTRIBUTES},
};
use aarch64_rt::InitialPagetable;
use arm_gic::{IntId, Trigger, gicv3::GicV3};
use arm_pl031::Rtc;
use core::ptr::NonNull;
use uart_16550::{Config, Uart16550, backend::MmioBackend};

/// Base address of the first 8250 UART.
const UART_BASE_ADDRESS: NonNull<u8> = NonNull::new(0x03f8 as _).unwrap();

/// Base address of the PL030 RTC.
const PL030_BASE_ADDRESS: *mut u32 = 0x2000 as _;

pub struct Crosvm {
    parts: Option<PlatformParts<Uart16550<MmioBackend>, Rtc>>,
}

impl Crosvm {
    const CONSOLE_IRQ: IntId = IntId::spi(0);

    /// Returns the initial hard-coded page table to use before the Rust code starts.
    pub const fn initial_idmap() -> InitialPagetable {
        let mut idmap = [0; 512];
        // 1 GiB of device mappings.
        idmap[0] = EL1_DEVICE_ATTRIBUTES.bits();
        // Another 1 GiB of device mappings.
        idmap[1] = EL1_DEVICE_ATTRIBUTES.bits() | 0x40000000;
        // 1 GiB of DRAM.
        idmap[2] = EL1_MEMORY_ATTRIBUTES.bits() | 0x80000000;
        InitialPagetable(idmap)
    }
}

impl Platform for Crosvm {
    type Console = Uart16550<MmioBackend>;
    type Rtc = Rtc;

    const RTC_IRQ: IntId = IntId::spi(1);

    unsafe fn create() -> Self {
        // SAFETY: There is a suitable UART at this base address on crosvm, and we have mapped it
        // with an appropriate device mapping. `create` is only called once so there are no aliases.
        let mut uart = unsafe { Uart16550::new_mmio(UART_BASE_ADDRESS, 1) }.unwrap();
        // Enables the RBR data available interrupt.
        uart.init(Config::default()).unwrap();
        Self {
            // SAFETY: The various base addresses are valid and mapped, and `create` is only called
            // once so there are no aliases.
            parts: Some(unsafe {
                PlatformParts {
                    console: uart,
                    rtc: Rtc::new(PL030_BASE_ADDRESS),
                }
            }),
        }
    }

    fn parts(&mut self) -> Option<PlatformParts<Uart16550<MmioBackend>, Rtc>> {
        self.parts.take()
    }

    fn setup_gic(gic: &mut GicV3) {
        gic.set_interrupt_priority(Self::CONSOLE_IRQ, None, 0x10)
            .unwrap();
        gic.set_trigger(Self::CONSOLE_IRQ, None, Trigger::Edge)
            .unwrap();
        gic.enable_interrupt(Self::CONSOLE_IRQ, None, true).unwrap();
        set_shared_irq_handler(
            Self::CONSOLE_IRQ,
            &Console::<Uart16550<MmioBackend>>::handle_irq,
        );
    }
}
