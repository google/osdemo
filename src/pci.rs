// Copyright 2024 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use crate::pagetable::{DEVICE_ATTRIBUTES, IdMap};
use aarch64_paging::paging::MemoryRegion;
use alloc::vec::Vec;
use buddy_system_allocator::FrameAllocator;
use core::{
    alloc::Layout,
    cmp::min,
    fmt::{self, Debug, Display, Formatter},
};
use flat_device_tree::{Fdt, node::FdtNode, standard_nodes::MemoryRange};
use log::{info, warn};
use virtio_drivers::transport::pci::bus::{
    BarInfo, Cam, Command, DeviceFunction, MemoryBarType, MmioCam, PciError, PciRoot,
};

pub const PCI_COMPATIBLE: &str = "pci-host-cam-generic";
pub const PCIE_COMPATIBLE: &str = "pci-host-ecam-generic";

#[derive(Debug)]
pub struct PciRootInfo {
    cam: Cam,
    mmio_base: *mut u8,
    ranges: Vec<PciRange>,
}

impl PciRootInfo {
    fn for_fdt_node(pci_node: FdtNode, cam: Cam, bar_range_limit: usize) -> Self {
        let region = pci_node.reg().next().unwrap();
        info!(
            "Reg: {:?}-{:#x}",
            region.starting_address,
            region.starting_address as usize + region.size.unwrap()
        );
        assert_eq!(region.size.unwrap(), cam.size() as usize);

        let mut ranges = Vec::new();
        for range in pci_node.ranges() {
            let mut range = PciRange::from(range);
            info!("PCI range {range}");
            if matches!(
                range.flags.range_type(),
                PciRangeType::Memory32 | PciRangeType::Memory64
            ) {
                if range.cpu_physical >= bar_range_limit {
                    warn!(
                        "Ignoring range outside page table size ({:#x} >= {:#x}).",
                        range.cpu_physical, bar_range_limit,
                    );
                    continue;
                } else {
                    // If the end of the range is outside our page table size, trim it down.
                    range.size = min(range.size, bar_range_limit - range.cpu_physical);
                }
            }
            ranges.push(range);
        }

        Self {
            cam,
            mmio_base: region.starting_address as *mut u8,
            ranges,
        }
    }

    /// Maps all the BAR ranges for this PCI root in the given IdMap.
    pub fn map_ranges(&self, idmap: &mut IdMap) {
        for range in &self.ranges {
            if matches!(
                range.flags.range_type(),
                PciRangeType::Memory32 | PciRangeType::Memory64
            ) {
                let memory_region = range.memory_region();
                info!("Mappping {memory_region}");
                idmap.map_range(&memory_region, DEVICE_ATTRIBUTES).unwrap();
            }
        }
    }

    /// Initialises and returns the PCI root represented by the given FDT node.
    ///
    /// Allocates BAR ranges for all devices on the root.
    ///
    /// # Safety
    ///
    /// This must only be called once per PCI root, to avoid creating aliases to the MMIO space. The
    /// root info must refer to a valid MMIO region which has already been mapped appropriately.
    pub unsafe fn init_pci(self) -> PciRoot<MmioCam<'static>> {
        // SAFETY: The caller promises that the pointer is to a valid MMIO region.
        let mut pci_root = PciRoot::new(unsafe { MmioCam::new(self.mmio_base, self.cam) });

        let mut allocator = PciBarAllocator::new(self.ranges);
        for (device_function, info) in pci_root.enumerate_bus(0) {
            info!("Initialising bars for {device_function} {info}");
            allocate_bars(&mut pci_root, &mut allocator, device_function).unwrap();
        }

        pci_root
    }
}

/// Finds all PCI and PCIE roots.
///
/// BAR ranges higher than the given address limit will be ignored.
pub fn find_pci_roots(fdt: &Fdt, bar_range_limit: usize) -> Vec<PciRootInfo> {
    let mut pci_roots = Vec::new();
    if let Some(pci_node) = fdt.find_compatible(&[PCI_COMPATIBLE]) {
        info!("PCI node: {}", pci_node.name);
        pci_roots.push(PciRootInfo::for_fdt_node(
            pci_node,
            Cam::MmioCam,
            bar_range_limit,
        ))
    } else if let Some(pcie_node) = fdt.find_compatible(&[PCIE_COMPATIBLE]) {
        info!("PCIE node: {}", pcie_node.name);
        pci_roots.push(PciRootInfo::for_fdt_node(
            pcie_node,
            Cam::Ecam,
            bar_range_limit,
        ))
    }
    pci_roots
}

/// Allocator for PCI BARs.
struct PciBarAllocator {
    memory32: FrameAllocator<32>,
    memory64: FrameAllocator<64>,
    prefetchable_memory64: FrameAllocator<64>,
}

impl PciBarAllocator {
    fn new(ranges: Vec<PciRange>) -> Self {
        let mut memory32 = FrameAllocator::new();
        let mut memory64 = FrameAllocator::new();
        let mut prefetchable_memory64 = FrameAllocator::new();
        for range in ranges {
            match range.flags.range_type() {
                PciRangeType::Memory32 => {
                    assert_eq!(range.cpu_physical, range.bus_address);
                    if range.flags.prefetchable() {
                        warn!("32-bit PCI range was marked as prefetchable");
                    }
                    memory32.add_frame(range.cpu_physical, range.cpu_physical + range.size);
                }
                PciRangeType::Memory64 => {
                    assert_eq!(range.cpu_physical, range.bus_address);
                    if range.bus_address + range.size < u32::MAX.try_into().unwrap() {
                        // If a 64-bit range is entirely within 32-bit address space then treat it
                        // as a 32-bit range. This is necessary for crosvm, which doesn't correctly
                        // provide any 32-bit ranges.
                        memory32.add_frame(range.cpu_physical, range.cpu_physical + range.size);
                    } else if range.flags.prefetchable() {
                        prefetchable_memory64
                            .add_frame(range.cpu_physical, range.cpu_physical + range.size);
                    } else {
                        memory64.add_frame(range.cpu_physical, range.cpu_physical + range.size);
                    }
                }
                _ => {}
            }
        }
        Self {
            memory32,
            memory64,
            prefetchable_memory64,
        }
    }

    fn allocate32(&mut self, layout: Layout) -> u32 {
        self.memory32
            .alloc_aligned(layout)
            .expect("Failed to allocate PCI BAR")
            .try_into()
            .unwrap()
    }

    fn allocate64(&mut self, layout: Layout, prefetchable: bool) -> u64 {
        if prefetchable {
            if let Some(allocation) = self.prefetchable_memory64.alloc_aligned(layout) {
                return allocation.try_into().unwrap();
            }
            // If prefetchable allocation fails then fall back to non-prefetchable.
        }

        if let Some(allocation) = self.memory64.alloc_aligned(layout) {
            allocation.try_into().unwrap()
        } else {
            // Fall back to 32-bit pool if the 64-bit pool fails.
            self.allocate32(layout).into()
        }
    }
}

/// A PCI root range, from which BARs can be allocated.
#[derive(Debug, Eq, PartialEq)]
pub struct PciRange {
    pub cpu_physical: usize,
    pub bus_address: usize,
    pub size: usize,
    pub flags: PciMemoryFlags,
}

impl PciRange {
    fn memory_region(&self) -> MemoryRegion {
        MemoryRegion::new(self.cpu_physical, self.cpu_physical + self.size)
    }
}

impl From<MemoryRange> for PciRange {
    fn from(range: MemoryRange) -> Self {
        Self {
            cpu_physical: range.parent_bus_address,
            bus_address: range.child_bus_address,
            size: range.size,
            flags: PciMemoryFlags(range.child_bus_address_hi),
        }
    }
}

impl Display for PciRange {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "PCI range CPU physical {:#0x}, bus_address {:#0x}, size {:#0x}, flags {}",
            self.cpu_physical, self.bus_address, self.size, self.flags,
        )
    }
}

/// Allocates all bars of the given PCI device function.
fn allocate_bars(
    pci_root: &mut PciRoot<MmioCam>,
    allocator: &mut PciBarAllocator,
    device_function: DeviceFunction,
) -> Result<(), PciError> {
    for (bar_index, info) in pci_root
        .bars(device_function)
        .unwrap()
        .into_iter()
        .enumerate()
    {
        let Some(info) = info else { continue };
        let bar_index = bar_index as u8;
        info!("BAR {bar_index}: {info}");
        match info {
            BarInfo::Memory {
                address_type,
                prefetchable,
                address: _,
                size,
            } => {
                if size > 0 {
                    let layout = Layout::from_size_align(size as usize, size as usize).unwrap();
                    match address_type {
                        MemoryBarType::Width32 => {
                            if prefetchable {
                                warn!("  32-bit BAR should not be marked prefetchable.");
                            }
                            let allocation = allocator.allocate32(layout);
                            info!("  allocated {allocation:#0x}");
                            pci_root.set_bar_32(device_function, bar_index, allocation);
                        }
                        MemoryBarType::Width64 => {
                            let allocation = allocator.allocate64(layout, prefetchable);
                            info!("  allocated {allocation:#0x}");
                            pci_root.set_bar_64(device_function, bar_index, allocation);
                        }
                        MemoryBarType::Below1MiB => {
                            unimplemented!("Below 1MiB BARs not supported.")
                        }
                    }
                }
            }
            BarInfo::IO { .. } => {
                warn!("Ignoring IO BAR");
            }
        }
    }

    // Enable the device to use its BARs.
    pci_root.set_command(
        device_function,
        Command::IO_SPACE | Command::MEMORY_SPACE | Command::BUS_MASTER,
    );

    Ok(())
}

/// Encodes memory flags of a PCI range
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct PciMemoryFlags(pub u32);

impl PciMemoryFlags {
    /// Returns whether this PCI range is relocatable.
    pub fn relocatable(self) -> bool {
        self.0 & 0x8000_0000 == 0
    }

    /// Returns whether this PCI range is prefetchable.
    pub fn prefetchable(self) -> bool {
        self.0 & 0x4000_0000 != 0
    }

    /// Returns the type of this PCI range.
    pub fn range_type(self) -> PciRangeType {
        PciRangeType::from((self.0 & 0x0300_0000) >> 24)
    }
}

impl Display for PciMemoryFlags {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "{:#x} ({} {:?})",
            self.0,
            if self.prefetchable() {
                "prefetchable"
            } else {
                "non-prefetchable"
            },
            self.range_type(),
        )
    }
}

/// Type of a PCI range
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PciRangeType {
    /// Range represents the PCI configuration space
    ConfigurationSpace,
    /// Range is on IO space
    IoSpace,
    /// Range is on 32-bit MMIO space
    Memory32,
    /// Range is on 64-bit MMIO space
    Memory64,
}

impl From<u32> for PciRangeType {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::ConfigurationSpace,
            1 => Self::IoSpace,
            2 => Self::Memory32,
            3 => Self::Memory64,
            _ => panic!("Tried to convert invalid range type {}", value),
        }
    }
}
