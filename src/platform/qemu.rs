use super::Platform;
use crate::drivers::pl011::Uart;
use log::error;
use smccc::{psci::system_off, Hvc};

pub const UART_BASE_ADDRESS: *mut u32 = 0x900_0000 as _;

/// The QEMU aarch64 virt platform.
pub struct Qemu {
    console: Option<Uart>,
}

impl Platform for Qemu {
    type Console = Uart;

    fn power_off() -> ! {
        system_off::<Hvc>().unwrap();
        error!("PSCI_SYSTEM_OFF returned unexpectedly");
        loop {}
    }

    unsafe fn create() -> Self {
        Self {
            console: Some(unsafe { Uart::new(UART_BASE_ADDRESS) }),
        }
    }

    fn console(&mut self) -> Option<Uart> {
        self.console.take()
    }
}
