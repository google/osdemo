mod qemu;

pub use qemu::Qemu as PlatformImpl;

/// Platform-specific code.
pub trait Platform {
    type Console;

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
