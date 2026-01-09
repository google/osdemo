// Copyright 2024 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

#![no_main]
#![no_std]
#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

mod apps;
mod console;
mod cpus;
pub mod devices;
pub mod drivers;
mod exceptions;
mod interrupts;
mod logger;
mod pagetable;
pub mod pci;
mod platform;
pub mod secondary_entry;
mod virtio;

use crate::{exceptions::current_el, interrupts::init_gic};
use aarch64_paging::paging::{MemoryRegion, PAGE_SIZE};
use aarch64_rt::entry;
use alloc::vec::Vec;
use apps::shell;
use buddy_system_allocator::{Heap, LockedHeap};
use core::{fmt::Write, ops::DerefMut};
use devices::Devices;
use dtoolkit::{
    Node, Property,
    fdt::{Fdt, FdtNode},
    standard::{NodeStandard, Reg},
};
use log::{LevelFilter, debug, error, info};
use pagetable::{DEVICE_ATTRIBUTES, IdMap, MEMORY_ATTRIBUTES, PAGETABLE};
use pci::{PCI_COMPATIBLE, PCIE_COMPATIBLE, find_pci_roots};
use platform::{Platform, PlatformImpl};
use smccc::{Hvc, Smc, psci::system_off};
use spin::{
    Once,
    mutex::{SpinMutex, SpinMutexGuard},
};
use virtio::{find_virtio_mmio_devices, find_virtio_pci_devices};

const LOG_LEVEL: LevelFilter = LevelFilter::Debug;

const PAGE_HEAP_SIZE: usize = 10 * PAGE_SIZE;
static PAGE_HEAP: SpinMutex<[u8; PAGE_HEAP_SIZE]> = SpinMutex::new([0; PAGE_HEAP_SIZE]);

const HEAP_SIZE: usize = 40 * PAGE_SIZE;
static HEAP: SpinMutex<[u8; HEAP_SIZE]> = SpinMutex::new([0; HEAP_SIZE]);

#[global_allocator]
static HEAP_ALLOCATOR: LockedHeap<32> = LockedHeap::new();

static FDT: Once<Fdt<'static>> = Once::new();

entry!(main);
fn main(x0: u64, _x1: u64, _x2: u64, _x3: u64) -> ! {
    let fdt_address = x0 as *const u8;
    // SAFETY: We only call `PlatformImpl::create` here, once on boot.
    let mut platform = unsafe { PlatformImpl::create() };
    let mut parts = platform.parts().unwrap();
    writeln!(parts.console, "DemoOS starting at EL{}...", current_el()).unwrap();
    let mut console = console::init(parts.console);
    logger::init(console.shared(), LOG_LEVEL).unwrap();
    info!("FDT address: {fdt_address:?}");
    // SAFETY: We trust that the FDT pointer we were given is valid, and this is the only time we
    // use it.
    let fdt = unsafe { Fdt::from_raw(fdt_address).unwrap() };
    info!("FDT size: {} bytes", fdt.data().len());
    debug!("FDT: {fdt}");
    for reserved in fdt.memory_reservations() {
        info!("Reserved memory: {reserved:?}");
    }
    FDT.call_once(|| fdt);

    // Give the allocator some memory to allocate.
    add_to_heap(
        HEAP_ALLOCATOR.lock().deref_mut(),
        SpinMutexGuard::leak(HEAP.try_lock().unwrap()).as_mut_slice(),
    );

    info!("Initialising page table...");
    let mut page_allocator = Heap::new();
    add_to_heap(
        &mut page_allocator,
        SpinMutexGuard::leak(PAGE_HEAP.try_lock().unwrap()).as_mut_slice(),
    );
    let mut idmap = IdMap::new(page_allocator);
    info!("IdMap size is {} GiB", idmap.size() / (1024 * 1024 * 1024));
    map_fdt_regions(&fdt, &mut idmap);

    let pci_roots_info = find_pci_roots(&fdt, idmap.size());
    for pci_root in &pci_roots_info {
        pci_root.map_ranges(&mut idmap);
    }

    debug!("Page table: {idmap:?}");

    info!("Activating page table...");
    // SAFETY: The page table maps all the memory we use, and we keep it until the end of the
    // program.
    unsafe {
        idmap.activate();
    }
    PAGETABLE.call_once(|| idmap);

    info!("Initialising GIC...");
    // SAFETY: We trust that the FDT is accurate, and we've already mapped things and activated the
    // pagetable.
    unsafe {
        init_gic(&fdt);
    }

    let mut devices = Devices::new(parts.rtc);
    // SAFETY: We only call this once, and we trust that the FDT is correct and the platform has
    // mapped all MMIO regions appropriately.
    unsafe { find_virtio_mmio_devices(&fdt, &mut devices) };

    let mut pci_roots = pci_roots_info
        .into_iter()
        // SAFETY: We only call this once, and `map_fdt_regions` mapped the MMIO regions.
        .map(|pci_root_info| unsafe { pci_root_info.init_pci() })
        .collect::<Vec<_>>();

    for pci_root in &mut pci_roots {
        find_virtio_pci_devices(pci_root, &mut devices);
    }

    shell::main(&mut console, &mut pci_roots, &mut devices, &fdt);

    info!("Powering off.");
    power_off();
}

/// Adds the given memory range to the given heap.
fn add_to_heap<const ORDER: usize>(heap: &mut Heap<ORDER>, range: &'static mut [u8]) {
    // SAFETY: The range we pass is valid because it comes from a mutable static reference, which it
    // effectively takes ownership of.
    unsafe {
        heap.init(range.as_mut_ptr() as usize, range.len());
    }
}

/// Maps memory and device regions from the FDT.
fn map_fdt_regions(fdt: &Fdt, idmap: &mut IdMap) {
    // Map memory.
    // TODO: Support multiple memory nodes, as allowed by the specification.
    for fdt_region in fdt.memory().unwrap().reg().unwrap().unwrap() {
        let region = fdt_to_pagetable_region(&fdt_region);
        let size = fdt_region.size::<u64>().unwrap();
        info!(
            "Mapping memory region {:?} from FDT ({} MiB)...",
            region,
            size / (1024 * 1024)
        );
        idmap.map_range(&region, MEMORY_ATTRIBUTES).unwrap();
    }

    // Map MMIO regions for devices.
    map_fdt_node_regions(&fdt.root(), idmap);
}

/// Maps MMIO regions for the device represented by the given FDT node and its children.
fn map_fdt_node_regions(node: &FdtNode, idmap: &mut IdMap) {
    if is_compatible(
        node,
        &[
            PCI_COMPATIBLE,
            PCIE_COMPATIBLE,
            "arm,gic-v3",
            "arm,gic-v3-its",
            "arm,pl011",
            "arm,pl031",
            "arm,pl061",
            "arm,primecell",
            "ns16550a",
            "virtio,mmio",
        ],
    ) {
        for fdt_region in node.reg().unwrap().unwrap() {
            let region = fdt_to_pagetable_region(&fdt_region);
            info!(
                "Mappping {} for {}, compatible={}",
                region,
                node.name(),
                node.compatible().unwrap().next().unwrap()
            );
            idmap.map_range(&region, DEVICE_ATTRIBUTES).unwrap();
        }
    } else if let Some(mut compatible) = node.compatible() {
        info!(
            "Ignoring {}, compatible={}",
            node.name(),
            compatible.next().unwrap()
        );
    } else {
        info!("Ignoring {}", node.name());
    }
    for child in node.children() {
        map_fdt_node_regions(&child, idmap);
    }
}

fn fdt_to_pagetable_region(region: &Reg) -> MemoryRegion {
    let address = region.address::<u64>().unwrap();
    let size = region.size::<u64>().unwrap();
    MemoryRegion::new(address as _, (address + size) as usize)
}

fn is_compatible(node: &FdtNode, with: &[&str]) -> bool {
    if let Some(mut compatible) = node.compatible() {
        compatible.any(|c| with.contains(&c))
    } else {
        false
    }
}

/// Powers off the system via PSCI.
fn power_off() -> ! {
    let result = if smc_for_psci() {
        system_off::<Smc>()
    } else {
        system_off::<Hvc>()
    };
    if let Err(e) = result {
        error!("PSCI_SYSTEM_OFF failed: {e}");
    } else {
        error!("PSCI_SYSTEM_OFF returned unexpectedly");
    }
    #[allow(clippy::empty_loop)]
    loop {}
}

/// Returns whether to use SMC calls for PSCI rather than HVCs.
fn smc_for_psci() -> bool {
    let Some(fdt) = FDT.get() else {
        return false;
    };
    let Some(psci_node) = fdt.find_node("/psci") else {
        return false;
    };
    let Some(method) = psci_node.property("method") else {
        return false;
    };
    method.value() == b"smc\0"
}
