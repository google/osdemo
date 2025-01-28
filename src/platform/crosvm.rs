// Copyright 2024 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use super::{Platform, PlatformParts};
use crate::{
    drivers::uart8250::Uart,
    pagetable::{InitialIdmap, DEVICE_ATTRIBUTES, MEMORY_ATTRIBUTES},
};
use arm_gic::gicv3::{GicV3, IntId};
use arm_pl031::Rtc;
use log::error;
use smccc::{psci::system_off, Hvc};

/// Base address of the first 8250 UART.
const UART_BASE_ADDRESS: *mut u8 = 0x03f8 as _;

/// Base address of the PL030 RTC.
const PL030_BASE_ADDRESS: *mut u32 = 0x2000 as _;

/// Base address of the GICv3 distributor.
const GICD_BASE_ADDRESS: *mut u64 = 0x3fff_0000 as _;

/// Base address of the GICv3 redistributor.
const GICR_BASE_ADDRESS: *mut u64 = 0x3ffd_0000 as _;

pub struct Crosvm {
    parts: Option<PlatformParts<Uart, Rtc>>,
}

impl Crosvm {
    /// Returns the initial hard-coded page table to use before the Rust code starts.
    pub const fn initial_idmap() -> InitialIdmap {
        let mut idmap = [0; 512];
        // 1 GiB of device mappings.
        idmap[0] = DEVICE_ATTRIBUTES.bits();
        // Another 1 GiB of device mappings.
        idmap[1] = DEVICE_ATTRIBUTES.bits() | 0x40000000;
        // 1 GiB of DRAM.
        idmap[2] = MEMORY_ATTRIBUTES.bits() | 0x80000000;
        InitialIdmap(idmap)
    }
}

impl Platform for Crosvm {
    type Console = Uart;
    type Rtc = Rtc;

    const RTC_IRQ: IntId = IntId::spi(2);

    fn power_off() -> ! {
        system_off::<Hvc>().unwrap();
        error!("PSCI_SYSTEM_OFF returned unexpectedly");
        #[allow(clippy::empty_loop)]
        loop {}
    }

    unsafe fn create() -> Self {
        Self {
            // SAFETY: The various base addresses are valid and mapped, and `create` is only called
            // once so there are no aliases.
            parts: Some(unsafe {
                PlatformParts {
                    console: Uart::new(UART_BASE_ADDRESS),
                    rtc: Rtc::new(PL030_BASE_ADDRESS),
                    gic: GicV3::new(GICD_BASE_ADDRESS, GICR_BASE_ADDRESS),
                }
            }),
        }
    }

    fn parts(&mut self) -> Option<PlatformParts<Uart, Rtc>> {
        self.parts.take()
    }
}
