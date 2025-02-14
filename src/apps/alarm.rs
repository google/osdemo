// Copyright 2024 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use crate::platform::{Platform, PlatformImpl};
use arm_gic::gicv3::{GicV3, IntId, Trigger};
use arm_pl031::Rtc;
use chrono::Duration;
use core::sync::atomic::{AtomicBool, Ordering};
use embedded_io::Write;
use log::info;

/// The RTC alarm IRQ has fired, and we have not yet cleared the interrupt.
static ALARM_FIRED: AtomicBool = AtomicBool::new(false);

/// Configures the RTC IRQ.
pub fn irq_setup(gic: &mut GicV3) {
    gic.set_interrupt_priority(PlatformImpl::RTC_IRQ, 0x80);
    gic.set_trigger(PlatformImpl::RTC_IRQ, Trigger::Level);
    gic.enable_interrupt(PlatformImpl::RTC_IRQ, true);
}

/// Handles an RTC IRQ.
pub fn irq_handle(_intid: IntId) {
    info!("RTC alarm");
    ALARM_FIRED.store(true, Ordering::SeqCst);
}

/// Finishes handling the alarm IRQ, ready to set another alarm in future.
pub fn irq_finish(rtc: &mut Rtc) {
    if ALARM_FIRED.swap(false, Ordering::SeqCst) {
        rtc.clear_interrupt();
        GicV3::end_interrupt(PlatformImpl::RTC_IRQ);
        info!("Alarm fired, clearing");
    }
}

/// Sets an alarm for 5 seconds in the future.
pub fn alarm<'a>(console: &mut impl Write, mut args: impl Iterator<Item = &'a str>, rtc: &mut Rtc) {
    irq_finish(rtc);

    let Some(delay) = args.next() else {
        writeln!(console, "Usage:").unwrap();
        writeln!(console, "  alarm <delay>").unwrap();
        return;
    };
    let Ok(delay) = delay.parse() else {
        writeln!(console, "Invalid delay time").unwrap();
        return;
    };

    let timestamp = rtc.get_time();
    let alarm_time = timestamp + Duration::seconds(delay);
    rtc.set_match(alarm_time).unwrap();
    rtc.enable_interrupt(true);
    writeln!(console, "Set alarm for {}", alarm_time).unwrap();
}
