mod qemu;

use embedded_io::{Read, ReadReady, Write, WriteReady};
pub use qemu::Qemu as PlatformImpl;

pub type Console = <PlatformImpl as Platform>::Console;

/// Platform-specific code.
pub trait Platform {
    type Console: Read + ReadReady + Send + Write + WriteReady;

    /// Powers off the system.
    fn power_off() -> !;

    /// Returns the primary console.
    ///
    /// This should return `Some` the first time it is called, but may return `None` on subsequent
    /// calls.
    ///
    /// # Safety
    ///
    /// This method must only be called once. Calling it multiple times would result in unsound
    /// mutable aliasing.
    unsafe fn console() -> Self::Console;
}
