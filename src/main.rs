#![no_main]
#![no_std]
#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(unsafe_op_in_unsafe_fn)]

mod apps;
mod console;
pub mod drivers;
mod exceptions;
mod logger;
mod pagetable;
pub mod pci;
mod platform;

use aarch64_paging::paging::{MemoryRegion, PAGE_SIZE};
use apps::shell;
use buddy_system_allocator::Heap;
use core::fmt::Write;
use flat_device_tree::{node::FdtNode, standard_nodes, Fdt};
use log::{debug, info, LevelFilter};
use pagetable::{IdMap, DEVICE_ATTRIBUTES, MEMORY_ATTRIBUTES};
use pci::{all_pci_roots, PCIE_COMPATIBLE, PCI_COMPATIBLE};
use platform::{Platform, PlatformImpl};

const PAGE_HEAP_SIZE: usize = 8 * PAGE_SIZE;
static mut PAGE_HEAP: [u8; PAGE_HEAP_SIZE] = [0; PAGE_HEAP_SIZE];

const LOG_LEVEL: LevelFilter = LevelFilter::Info;

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
    map_fdt_regions(&fdt, &mut idmap);

    info!("Activating page table...");
    // SAFETY: The page table maps all the memory we use, and we keep it until the end of the
    // program.
    unsafe {
        idmap.activate();
    }

    // SAFETY: We only call this once, and `map_fdt_regions` mapped the MMIO regions.
    let mut pci_roots = unsafe { all_pci_roots(&fdt) };

    shell::main(&mut console, &mut parts.rtc, &mut parts.gic, &mut pci_roots);

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
