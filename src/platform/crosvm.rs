// Copyright 2024 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use super::{Platform, PlatformParts};
use crate::{
    console::Console,
    drivers::uart8250::Uart,
    interrupts::set_irq_handler,
    pagetable::{DEVICE_ATTRIBUTES, MEMORY_ATTRIBUTES},
};
use aarch64_rt::InitialPagetable;
use arm_gic::{IntId, Trigger, gicv3::GicV3};
use arm_pl031::Rtc;
use log::error;
use smccc::{Hvc, psci::system_off};

/// Base address of the first 8250 UART.
const UART_BASE_ADDRESS: *mut u8 = 0x03f8 as _;

/// Base address of the PL030 RTC.
const PL030_BASE_ADDRESS: *mut u32 = 0x2000 as _;

pub struct Crosvm {
    parts: Option<PlatformParts<Uart, Rtc>>,
}

impl Crosvm {
    const CONSOLE_IRQ: IntId = IntId::spi(0);

    /// Returns the initial hard-coded page table to use before the Rust code starts.
    pub const fn initial_idmap() -> InitialPagetable {
        let mut idmap = [0; 512];
        // 1 GiB of device mappings.
        idmap[0] = DEVICE_ATTRIBUTES.bits();
        // Another 1 GiB of device mappings.
        idmap[1] = DEVICE_ATTRIBUTES.bits() | 0x40000000;
        // 1 GiB of DRAM.
        idmap[2] = MEMORY_ATTRIBUTES.bits() | 0x80000000;
        InitialPagetable(idmap)
    }
}

impl Platform for Crosvm {
    type Console = Uart;
    type Rtc = Rtc;

    const RTC_IRQ: IntId = IntId::spi(1);

    fn power_off() -> ! {
        system_off::<Hvc>().unwrap();
        error!("PSCI_SYSTEM_OFF returned unexpectedly");
        #[allow(clippy::empty_loop)]
        loop {}
    }

    unsafe fn create() -> Self {
        // SAFETY: There is a suitable UART at this base address on crosvm, and we have mapped it
        // with an appropriate device mapping. `create` is only called once so there are no aliases.
        let mut uart = unsafe { Uart::new(UART_BASE_ADDRESS) };
        // Enable the RBR data available interrupt.
        uart.enable_interrupts(0b0001);
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

    fn parts(&mut self) -> Option<PlatformParts<Uart, Rtc>> {
        self.parts.take()
    }

    fn setup_gic(gic: &mut GicV3) {
        gic.set_interrupt_priority(Self::CONSOLE_IRQ, None, 0x10);
        gic.set_trigger(Self::CONSOLE_IRQ, None, Trigger::Edge);
        gic.enable_interrupt(Self::CONSOLE_IRQ, None, true);
        set_irq_handler(Self::CONSOLE_IRQ, &Console::<Uart>::handle_irq);
    }
}
