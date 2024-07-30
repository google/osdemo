use crate::is_compatible;
use core::{mem::size_of, ptr::NonNull};
use flat_device_tree::Fdt;
use log::{debug, error, info};
use virtio_drivers::transport::{
    mmio::{MmioError, MmioTransport, VirtIOHeader},
    Transport,
};

const VIRTIO_MMIO_COMPATIBLE: &str = "virtio,mmio";

pub fn find_virtio_mmio_devices(fdt: &Fdt) {
    for node in fdt.all_nodes() {
        if is_compatible(&node, &[VIRTIO_MMIO_COMPATIBLE]) {
            debug!("Found VirtIO MMIO device {}", node.name);
            if let Some(region) = node.reg().next() {
                let region_size = region.size.unwrap_or(0);
                if region_size < size_of::<VirtIOHeader>() {
                    error!(
                        "VirtIO MMIO device {} region smaller than VirtIO header size ({} < {})",
                        node.name,
                        region_size,
                        size_of::<VirtIOHeader>()
                    );
                } else {
                    let header =
                        NonNull::new(region.starting_address as *mut VirtIOHeader).unwrap();
                    match unsafe { MmioTransport::new(header) } {
                        Err(MmioError::ZeroDeviceId) => {
                            debug!("Ignoring VirtIO device with zero device ID.");
                        }
                        Err(e) => {
                            error!("Error creating VirtIO transport: {}", e);
                        }
                        Ok(mut transport) => {
                            info!(
                                "Detected virtio MMIO device with device type {:?}, vendor ID {:#x}, version {:?}, features {:#018x}",
                                transport.device_type(),
                                transport.vendor_id(),
                                transport.version(),
                                transport.read_device_features(),
                            );
                        }
                    }
                }
            } else {
                error!("VirtIO MMIO device {} missing region", node.name);
            }
        }
    }
}
