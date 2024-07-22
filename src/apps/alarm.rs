use crate::platform::{Platform, PlatformImpl};
use arm_gic::gicv3::{GicV3, Trigger};
use arm_pl031::Rtc;
use chrono::Duration;
use core::{
    fmt::Write,
    sync::atomic::{AtomicBool, Ordering},
};
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
pub fn irq_handle() {
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
pub fn alarm(console: &mut impl Write, rtc: &mut Rtc) {
    irq_finish(rtc);

    let timestamp = rtc.get_time();
    let alarm_time = timestamp + Duration::seconds(4);
    rtc.set_match(alarm_time).unwrap();
    rtc.enable_interrupt(true);
    writeln!(console, "Set alarm for {}", alarm_time).unwrap();
}
