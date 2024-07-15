mod pl011;

use super::Platform;
use pl011::Uart;
use smccc::{psci::system_off, Hvc};

pub const UART_BASE_ADDRESS: *mut u32 = 0x900_0000 as _;

/// The QEMU aarch64 virt platform.
pub struct Qemu;

impl Platform for Qemu {
    type Console = Uart;

    fn power_off() -> ! {
        system_off::<Hvc>().unwrap();
        panic!("PSCI_SYSTEM_OFF returned unexpectedly");
    }

    unsafe fn console() -> Uart {
        unsafe { Uart::new(UART_BASE_ADDRESS) }
    }
}
