use super::Platform;
use smccc::{psci::system_off, Hvc};

/// The QEMU aarch64 virt platform.
pub struct Qemu;

impl Platform for Qemu {
    fn power_off() -> ! {
        system_off::<Hvc>().unwrap();
        panic!("PSCI_SYSTEM_OFF returned unexpectedly");
    }
}
