// Copyright 2024 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

mod crosvm;
mod qemu;

use arm_gic::{gicv3::GicV3, IntId};
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

    /// The IRQ used by the RTC.
    const RTC_IRQ: IntId;

    /// Powers off the system.
    fn power_off() -> !;

    /// Creates an instance of the platform.
    ///
    /// # Safety
    ///
    /// This method must only be called once. Calling it multiple times would result in unsound
    /// mutable aliasing.
    unsafe fn create() -> Self;

    /// Returns the drivers provided by the platform.
    ///
    /// This should return `Some` the first time it is called, but may return `None` on subsequent
    /// calls.
    fn parts(&mut self) -> Option<PlatformParts<Self::Console, Self::Rtc>>;

    fn setup_gic(_gic: &mut GicV3) {}
}

/// The drivers provided by each platform.
pub struct PlatformParts<Console, Rtc> {
    /// The primary console.
    pub console: Console,
    /// The GIC.
    pub gic: GicV3<'static>,
    /// The real-time clock.
    pub rtc: Rtc,
}
