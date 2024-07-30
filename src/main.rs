#![no_main]
#![no_std]
#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

mod apps;
mod console;
pub mod devices;
pub mod drivers;
mod exceptions;
mod logger;
mod pagetable;
pub mod pci;
mod platform;
mod virtio;

use aarch64_paging::paging::{MemoryRegion, PAGE_SIZE};
use alloc::vec::Vec;
use apps::shell;
use buddy_system_allocator::{Heap, LockedHeap};
use core::fmt::Write;
use devices::Devices;
use flat_device_tree::{node::FdtNode, standard_nodes, Fdt};
use log::{debug, info, LevelFilter};
use pagetable::{IdMap, DEVICE_ATTRIBUTES, MEMORY_ATTRIBUTES};
use pci::{find_pci_roots, PCIE_COMPATIBLE, PCI_COMPATIBLE};
use platform::{Platform, PlatformImpl};
use virtio::{find_virtio_mmio_devices, find_virtio_pci_devices};

const LOG_LEVEL: LevelFilter = LevelFilter::Info;

const PAGE_HEAP_SIZE: usize = 10 * PAGE_SIZE;
static mut PAGE_HEAP: [u8; PAGE_HEAP_SIZE] = [0; PAGE_HEAP_SIZE];

const HEAP_SIZE: usize = 10 * PAGE_SIZE;
static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

#[global_allocator]
static HEAP_ALLOCATOR: LockedHeap<32> = LockedHeap::new();

#[no_mangle]
extern "C" fn main(fdt_address: *const u8) {
    // SAFETY: We only call `PlatformImpl::create` here, once on boot.
    let mut platform = unsafe { PlatformImpl::create() };
    let mut parts = platform.parts().unwrap();
    writeln!(parts.console, "DemoOS starting...").unwrap();
    let mut console = console::init(parts.console);
    logger::init(console, LOG_LEVEL).unwrap();
    info!("FDT address: {:?}", fdt_address);
    // SAFETY: We trust that the FDT pointer we were given is valid, and this is the only time we
    // use it.
    let fdt = unsafe { Fdt::from_ptr(fdt_address).unwrap() };
    info!("FDT size: {} bytes", fdt.total_size());
    debug!("FDT: {:?}", fdt);
    for reserved in fdt.memory_reservations() {
        info!("Reserved memory: {:?}", reserved);
    }

    // SAFETY: `HEAP` is only used here and `entry` is only called once.
    unsafe {
        // Give the allocator some memory to allocate.
        HEAP_ALLOCATOR
            .lock()
            .init(HEAP.as_mut_ptr() as usize, HEAP.len());
    }

    info!("Initialising GIC...");
    parts.gic.setup();

    info!("Initialising page table...");
    let mut page_allocator = Heap::new();
    // SAFETY: We only do this once, as `Once::call_once` guarantees. Nothing else accesses the
    // `HEAP` mutable static.
    unsafe {
        page_allocator.init(PAGE_HEAP.as_mut_ptr() as usize, PAGE_HEAP.len());
    }
    let mut idmap = IdMap::new(page_allocator);
    info!("IdMap size is {} GiB", idmap.size() / (1024 * 1024 * 1024));
    map_fdt_regions(&fdt, &mut idmap);

    let pci_roots_info = find_pci_roots(&fdt, idmap.size());
    for pci_root in &pci_roots_info {
        pci_root.map_ranges(&mut idmap);
    }

    info!("Activating page table...");
    // SAFETY: The page table maps all the memory we use, and we keep it until the end of the
    // program.
    unsafe {
        idmap.activate();
    }

    let mut devices = Devices::new(parts.rtc);
    find_virtio_mmio_devices(&fdt, &mut devices);

    // SAFETY: We only call this once, and `map_fdt_regions` mapped the MMIO regions.
    let mut pci_roots = pci_roots_info
        .into_iter()
        .map(|pci_root_info| unsafe { pci_root_info.init_pci() })
        .collect::<Vec<_>>();

    for pci_root in &mut pci_roots {
        find_virtio_pci_devices(pci_root, &mut devices);
    }

    shell::main(&mut console, &mut parts.gic, &mut pci_roots, &mut devices);

    info!("Powering off.");
    PlatformImpl::power_off();
}

/// Maps memory and device regions from the FDT.
fn map_fdt_regions(fdt: &Fdt, idmap: &mut IdMap) {
    // Map memory.
    // TODO: Support multiple memory nodes, as allowed by the specification.
    for fdt_region in fdt.memory().unwrap().regions() {
        let region = fdt_to_pagetable_region(&fdt_region);
        info!(
            "Mapping memory region {:?} from FDT ({} MiB)...",
            region,
            fdt_region.size.unwrap() / (1024 * 1024)
        );
        idmap.map_range(&region, MEMORY_ATTRIBUTES).unwrap();
    }

    // Map MMIO regions for devices.
    for node in fdt.all_nodes() {
        if is_compatible(
            &node,
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
            for fdt_region in node.reg() {
                let region = fdt_to_pagetable_region(&fdt_region);
                info!(
                    "Mappping {} for {}, compatible={}",
                    region,
                    node.name,
                    node.compatible().unwrap().first().unwrap()
                );
                idmap.map_range(&region, DEVICE_ATTRIBUTES).unwrap();
            }
        } else if let Some(compatible) = node.compatible() {
            info!(
                "Ignoring {}, compatible={}",
                node.name,
                compatible.first().unwrap()
            );
        } else {
            info!("Ignoring {}", node.name);
        }
    }
}

fn fdt_to_pagetable_region(region: &standard_nodes::MemoryRegion) -> MemoryRegion {
    MemoryRegion::new(
        region.starting_address as _,
        region.starting_address as usize + region.size.unwrap(),
    )
}

fn is_compatible(node: &FdtNode, with: &[&str]) -> bool {
    if let Some(compatible) = node.compatible() {
        compatible.all().any(|c| with.contains(&c))
    } else {
        false
    }
}
