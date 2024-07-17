mod crosvm;
mod qemu;

#[cfg(platform = "crosvm")]
pub use crosvm::Crosvm as PlatformImpl;
use embedded_io::{Read, ReadReady, Write, WriteReady};
#[cfg(platform = "qemu")]
pub use qemu::Qemu as PlatformImpl;

pub type ConsoleImpl = <PlatformImpl as Platform>::Console;

/// Platform-specific code.
pub trait Platform {
    type Console: Read + ReadReady + Send + Write + WriteReady;
    type Rtc;

    /// Powers off the system.
    fn power_off() -> !;

    /// Creates an instance of the platform.
    ///
    /// # Safety
    ///
    /// This method must only be called once. Calling it multiple times would result in unsound
    /// mutable aliasing.
    unsafe fn create() -> Self;

    /// Returns the primary console.
    ///
    /// This should return `Some` the first time it is called, but may return `None` on subsequent
    /// calls.
    fn console(&mut self) -> Option<Self::Console>;

    /// Returns the real-time clock.
    ///
    /// This should return `Some` the first time it is called, but may return `None` on subsequent
    /// calls.
    fn rtc(&mut self) -> Option<Self::Rtc>;
}
