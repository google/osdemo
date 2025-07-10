// Copyright 2025 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use crate::{
    cpus::{PerCoreState, current_cpu_index, new_per_core_state_with_default},
    platform::{Platform, PlatformImpl},
};
use alloc::collections::btree_map::BTreeMap;
use arm_gic::{
    IntId,
    gicv3::{
        GicV3, InterruptGroup,
        registers::{Gicd, GicrSgi},
    },
};
use flat_device_tree::Fdt;
use log::{info, trace};
use percore::{ExceptionLock, exception_free};
use spin::{Once, mutex::SpinMutex};

type IrqHandler = &'static (dyn Fn(IntId) + Sync);

static SHARED_IRQ_HANDLERS: ExceptionLock<SpinMutex<BTreeMap<IntId, IrqHandler>>> =
    ExceptionLock::new(SpinMutex::new(BTreeMap::new()));
static PRIVATE_IRQ_HANDLERS: PerCoreState<BTreeMap<IntId, IrqHandler>> =
    new_per_core_state_with_default();

pub static GIC: Once<SpinMutex<GicV3>> = Once::new();

/// Sets the IRQ handler for the given interrupt ID to the given function, on all cores.
///
/// Returns the handler that was previously set, if any.
pub fn set_shared_irq_handler(intid: IntId, handler: IrqHandler) -> Option<IrqHandler> {
    trace!("Setting shared IRQ handler for {:?}", intid);
    exception_free(|token| {
        assert!(
            !PRIVATE_IRQ_HANDLERS
                .get()
                .borrow(token)
                .borrow()
                .contains_key(&intid),
            "Private IRQ handler already exists for {intid:?}",
        );
        SHARED_IRQ_HANDLERS
            .borrow(token)
            .lock()
            .insert(intid, handler)
    })
}

/// Removes the shared IRQ handler for the given interrupt ID.
///
/// Returns the handler that was previously set, if any.
pub fn remove_shared_irq_handler(intid: IntId) -> Option<IrqHandler> {
    trace!("Removing shared IRQ handler for {:?}", intid);
    exception_free(|token| SHARED_IRQ_HANDLERS.borrow(token).lock().remove(&intid))
}

/// Sets the IRQ handler for the given interrupt ID to the given function, on the current core only.
///
/// Returns the handler that was previously set, if any.
pub fn set_private_irq_handler(intid: IntId, handler: IrqHandler) -> Option<IrqHandler> {
    trace!("Setting private IRQ handler for {:?}", intid);
    exception_free(|token| {
        assert!(
            !SHARED_IRQ_HANDLERS
                .borrow(token)
                .lock()
                .contains_key(&intid),
            "Private IRQ handler already exists for {intid:?}",
        );
        PRIVATE_IRQ_HANDLERS
            .get()
            .borrow_mut(token)
            .insert(intid, handler)
    })
}

/// Removes the private IRQ handler for the given interrupt ID.
///
/// Returns the handler that was previously set, if any.
pub fn remove_private_irq_handler(intid: IntId) -> Option<IrqHandler> {
    trace!("Removing private IRQ handler for {:?}", intid);
    exception_free(|token| PRIVATE_IRQ_HANDLERS.get().borrow_mut(token).remove(&intid))
}

/// Asks the GIC what interrupt is pending and then calls the appropriate handler.
///
/// This should be called when there is an irq_current exception.
///
/// Panics if there is no no pending interrupt, or no registered handler for the pending interrupt.
pub fn handle_irq() {
    let intid =
        GicV3::get_and_acknowledge_interrupt(InterruptGroup::Group1).expect("No pending interrupt");
    trace!("IRQ: {:?}", intid);
    exception_free(|token| {
        if let Some(handler) = PRIVATE_IRQ_HANDLERS
            .get()
            .borrow(token)
            .borrow()
            .get(&intid)
        {
            handler(intid);
        } else if let Some(handler) = SHARED_IRQ_HANDLERS.borrow(token).lock().get(&intid) {
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
