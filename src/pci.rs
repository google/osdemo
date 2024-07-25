use crate::{
    fdt_to_pagetable_region,
    pagetable::{IdMap, DEVICE_ATTRIBUTES},
};
use flat_device_tree::{node::FdtNode, Fdt};
use log::info;
use virtio_drivers::transport::pci::bus::{Cam, PciRoot};

/// Finds and initialises the first PCI or PCIE root, if any.
pub fn init_first_pci(fdt: &Fdt, idmap: &mut IdMap) -> Option<PciRoot> {
    if let Some(pci_node) = fdt.find_compatible(&["pci-host-cam-generic"]) {
        info!("PCI node: {}", pci_node.name);
        Some(init_pci(pci_node, Cam::MmioCam, idmap))
    } else if let Some(pcie_node) = fdt.find_compatible(&["pci-host-ecam-generic"]) {
        info!("PCIE node: {}", pcie_node.name);
        Some(init_pci(pcie_node, Cam::Ecam, idmap))
    } else {
        None
    }
}

/// Maps the MMIO region for the PCI root represented by the given FDT node, and initialises and
/// returns it.
fn init_pci(pci_node: FdtNode, cam: Cam, idmap: &mut IdMap) -> PciRoot {
    let mut regions = pci_node.reg();
    let region = regions.next().unwrap();
    info!(
        "Reg: {:?}-{:#x}",
        region.starting_address,
        region.starting_address as usize + region.size.unwrap()
    );
    assert_eq!(region.size.unwrap(), cam.size() as usize);
    idmap
        .map_range(&fdt_to_pagetable_region(&region), DEVICE_ATTRIBUTES)
        .unwrap();
    // SAFETY: The FDT promises that the pointer is to a valid MMIO region.
    unsafe { PciRoot::new(region.starting_address as *mut u8, cam) }
}
