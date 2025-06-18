// Copyright 2025 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use crate::{
    cpus::current_cpu_index,
    platform::{Platform, PlatformImpl},
};
use alloc::collections::btree_map::BTreeMap;
use arm_gic::{
    IntId,
    gicv3::{
        GicV3,
        registers::{Gicd, GicrSgi},
    },
};
use flat_device_tree::Fdt;
use log::{info, trace};
use percore::{ExceptionLock, exception_free};
use spin::{Once, mutex::SpinMutex};

type IrqHandler = &'static (dyn Fn(IntId) + Sync);

static IRQ_HANDLER: ExceptionLock<SpinMutex<BTreeMap<IntId, IrqHandler>>> =
    ExceptionLock::new(SpinMutex::new(BTreeMap::new()));

pub static GIC: Once<SpinMutex<GicV3>> = Once::new();

/// Sets the IRQ handler for the given interrupt ID to the given function.
///
/// Returns the handler that was previously set, if any.
pub fn set_irq_handler(intid: IntId, handler: IrqHandler) -> Option<IrqHandler> {
    trace!("Setting IRQ handler for {:?}", intid);
    exception_free(|token| IRQ_HANDLER.borrow(token).lock().insert(intid, handler))
}

/// Removes the IRQ handler for the given interrupt ID.
///
/// Returns the handler that was previously set, if any.
pub fn remove_irq_handler(intid: IntId) -> Option<IrqHandler> {
    trace!("Removing IRQ handler for {:?}", intid);
    exception_free(|token| IRQ_HANDLER.borrow(token).lock().remove(&intid))
}

/// Asks the GIC what interrupt is pending and then calls the appropriate handler.
///
/// This should be called when there is an irq_current exception.
///
/// Panics if there is no no pending interrupt, or no registered handler for the pending interrupt.
pub fn handle_irq() {
    let intid = GicV3::get_and_acknowledge_interrupt().expect("No pending interrupt");
    trace!("IRQ: {:?}", intid);
    exception_free(|token| {
        if let Some(handler) = IRQ_HANDLER.borrow(token).lock().get(&intid) {
            handler(intid);
        } else {
            panic!("Unexpected IRQ {:?} with no handler", intid);
        }
    });
}

/// Finds a GICv3 in the given device tree and constructs a driver for it.
///
/// # Safety
///
/// This must only be called once, to avoid creating multiple drivers with aliases to the same GIC.
/// The given FDT must accurately reflect the platform, and the GIC device must already be mapped
/// in the pagetable and not used anywhere else.
unsafe fn make_gic(fdt: &Fdt) -> Option<GicV3<'static>> {
    let cpu_count = fdt.cpus().count();

    let node = fdt.find_compatible(&["arm,gic-v3"])?;
    info!("Found GIC FDT node {}", node.name);
    let mut reg = node.reg();
    let gicd_region = reg.next().expect("GICD region missing");
    let gicr_region = reg.next().expect("GICR region missing");
    info!("  GICD: {:?}", gicd_region);
    info!("  GICR: {:?}", gicr_region);
    info!(
        "  GICR space for {} CPUs",
        gicr_region.size.unwrap() / size_of::<GicrSgi>()
    );
    assert_eq!(gicd_region.size.unwrap(), size_of::<Gicd>());
    assert!(gicr_region.size.unwrap() >= size_of::<GicrSgi>() * cpu_count);
    // SAFETY: Our caller promised that the device tree is accurate and we are only called once.
    let gic = unsafe {
        GicV3::new(
            gicd_region.starting_address as _,
            gicr_region.starting_address as _,
            cpu_count,
            false,
        )
    };

    Some(gic)
}

/// Finds a GICv3 in the device tree, creates a driver for it, initialises it ready to start
/// handling interrupts, and stores it for later access.
///
/// # Safety
///
/// The given FDT must accurately reflect the platform, and the GIC device must already be mapped
/// in the pagetable and not used anywhere else.
pub unsafe fn init_gic(fdt: &Fdt) {
    GIC.call_once(|| {
        // SAFETY: Our caller promised that the FDT is accurate, and the call_once ensures that this
        // isn't called more than once.
        let mut gic = unsafe { make_gic(fdt) }.expect("No GIC found in FDT");

        gic.setup(0);
        PlatformImpl::setup_gic(&mut gic);

        SpinMutex::new(gic)
    });
}

/// Initialises the GIC on a secondary CPU core which has just come online.
///
/// This will panic if `init_gic` has not already been called on the primary CPU core.
pub fn secondary_init_gic() {
    let cpu = current_cpu_index();
    {
        let mut gic = GIC.get().unwrap().lock();
        gic.init_cpu(cpu);
    }
    GicV3::enable_group1(true);
    GicV3::set_priority_mask(0xff);
}
