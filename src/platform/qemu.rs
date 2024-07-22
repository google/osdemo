use super::Platform;
use crate::drivers::pl011::Uart;
use arm_gic::gicv3::GicV3;
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
    console: Option<Uart>,
    rtc: Option<Rtc>,
    gic: Option<GicV3>,
}

impl Platform for Qemu {
    type Console = Uart;
    type Rtc = Rtc;

    fn power_off() -> ! {
        system_off::<Hvc>().unwrap();
        error!("PSCI_SYSTEM_OFF returned unexpectedly");
        loop {}
    }

    unsafe fn create() -> Self {
        Self {
            console: Some(unsafe { Uart::new(UART_BASE_ADDRESS) }),
            rtc: Some(unsafe { Rtc::new(PL031_BASE_ADDRESS) }),
            gic: Some(unsafe { GicV3::new(GICD_BASE_ADDRESS, GICR_BASE_ADDRESS) }),
        }
    }

    fn console(&mut self) -> Option<Uart> {
        self.console.take()
    }

    fn rtc(&mut self) -> Option<Rtc> {
        self.rtc.take()
    }

    fn gic(&mut self) -> Option<GicV3> {
        self.gic.take()
    }
}
