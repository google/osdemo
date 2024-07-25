use super::{Platform, PlatformParts};
use crate::{
    drivers::pl011::Uart,
    pagetable::{IdMap, DEVICE_ATTRIBUTES},
};
use aarch64_paging::{paging::MemoryRegion, MapError};
use arm_gic::gicv3::{GicV3, IntId};
use arm_pl031::Rtc;
use log::error;
use smccc::{psci::system_off, Hvc};

/// Base address of the first PL011 UART.
const UART_BASE_ADDRESS: *mut u32 = 0x900_0000 as _;

/// Base address of the PL031 RTC.
const PL031_BASE_ADDRESS: *mut u32 = 0x901_0000 as _;

/// Base address of the GICv3 distributor.
const GICD_BASE_ADDRESS: *mut u64 = 0x800_0000 as _;

/// Base address of the GICv3 redistributor.
const GICR_BASE_ADDRESS: *mut u64 = 0x80A_0000 as _;

/// The QEMU aarch64 virt platform.
pub struct Qemu {
    parts: Option<PlatformParts<Uart, Rtc>>,
}

impl Platform for Qemu {
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
                    rtc: Rtc::new(PL031_BASE_ADDRESS),
                    gic: GicV3::new(GICD_BASE_ADDRESS, GICR_BASE_ADDRESS),
                }
            }),
        }
    }

    fn parts(&mut self) -> Option<PlatformParts<Uart, Rtc>> {
        self.parts.take()
    }

    fn map_pages(&self, idmap: &mut IdMap) -> Result<(), MapError> {
        idmap.map_range(
            &MemoryRegion::new(0x0000_0000, 0x4000_0000),
            DEVICE_ATTRIBUTES,
        )?;
        Ok(())
    }
}
