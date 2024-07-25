use arrayvec::ArrayVec;
use flat_device_tree::{node::FdtNode, Fdt};
use log::info;
use virtio_drivers::transport::pci::bus::{Cam, PciRoot};

pub const PCI_COMPATIBLE: &str = "pci-host-cam-generic";
pub const PCIE_COMPATIBLE: &str = "pci-host-ecam-generic";
const MAX_PCI_ROOTS: usize = 2;

pub type PciRoots = ArrayVec<PciRoot, MAX_PCI_ROOTS>;

/// Finds and initialises all PCI and PCIE roots.
///
/// # Safety
///
/// This must only be called once, to avoid creating aliases to the PCI MMIO space. The FDT must be
/// correct, and the MMIO regions must already have been mapped appropriately.
pub unsafe fn all_pci_roots(fdt: &Fdt) -> PciRoots {
    let mut pci_roots = ArrayVec::new();
    if let Some(pci_node) = fdt.find_compatible(&[PCI_COMPATIBLE]) {
        info!("PCI node: {}", pci_node.name);
        // SAFETY: Our caller promises to only call us once, and this is the only time we initialise
        // this PCI root.
        pci_roots.push(unsafe { init_pci(pci_node, Cam::MmioCam) })
    } else if let Some(pcie_node) = fdt.find_compatible(&[PCIE_COMPATIBLE]) {
        info!("PCIE node: {}", pcie_node.name);
        // SAFETY: Our caller promises to only call us once, and this is the only time we initialise
        // this PCI root.
        pci_roots.push(unsafe { init_pci(pcie_node, Cam::Ecam) })
    }
    pci_roots
}

/// Initialises and returns the PCI root represented by the given FDT node.
///
/// # Safety
///
/// This must only be called once per PCI root, to avoid creating aliases to the MMIO space. The
/// MMIO regions must already have been mapped appropriately.
unsafe fn init_pci(pci_node: FdtNode, cam: Cam) -> PciRoot {
    let mut regions = pci_node.reg();
    let region = regions.next().unwrap();
    info!(
        "Reg: {:?}-{:#x}",
        region.starting_address,
        region.starting_address as usize + region.size.unwrap()
    );
    assert_eq!(region.size.unwrap(), cam.size() as usize);
    // SAFETY: The FDT promises that the pointer is to a valid MMIO region.
    unsafe { PciRoot::new(region.starting_address as *mut u8, cam) }
}
