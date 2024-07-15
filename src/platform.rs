mod qemu;

pub use qemu::Qemu as PlatformImpl;

/// Platform-specific code.
pub trait Platform {
    /// Powers off the system.
    fn power_off() -> !;
}
