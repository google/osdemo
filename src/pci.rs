use flat_device_tree::{node::FdtNode, Fdt};
use log::info;
use virtio_drivers::transport::pci::bus::{Cam, PciRoot};

pub const PCI_COMPATIBLE: &str = "pci-host-cam-generic";
pub const PCIE_COMPATIBLE: &str = "pci-host-ecam-generic";

/// Finds and initialises the first PCI or PCIE root, if any.
pub fn init_first_pci(fdt: &Fdt) -> Option<PciRoot> {
    if let Some(pci_node) = fdt.find_compatible(&[PCI_COMPATIBLE]) {
        info!("PCI node: {}", pci_node.name);
        Some(init_pci(pci_node, Cam::MmioCam))
    } else if let Some(pcie_node) = fdt.find_compatible(&[PCIE_COMPATIBLE]) {
        info!("PCIE node: {}", pcie_node.name);
        Some(init_pci(pcie_node, Cam::Ecam))
    } else {
        None
    }
}

/// Maps the MMIO region for the PCI root represented by the given FDT node, and initialises and
/// returns it.
fn init_pci(pci_node: FdtNode, cam: Cam) -> PciRoot {
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
