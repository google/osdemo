use super::Platform;
use crate::drivers::uart8250::Uart;
use arm_pl031::Rtc;
use log::error;
use smccc::{psci::system_off, Hvc};

/// The base address of the first 8250 UART.
const UART_BASE_ADDRESS: *mut u8 = 0x03f8 as _;

/// Base address of the PL030 RTC.
const PL030_BASE_ADDRESS: *mut u32 = 0x2000 as _;

pub struct Crosvm {
    console: Option<Uart>,
    rtc: Option<Rtc>,
}

impl Platform for Crosvm {
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
            rtc: Some(unsafe { Rtc::new(PL030_BASE_ADDRESS) }),
        }
    }

    fn console(&mut self) -> Option<Uart> {
        self.console.take()
    }

    fn rtc(&mut self) -> Option<Rtc> {
        self.rtc.take()
    }
}
