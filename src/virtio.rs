// Copyright 2024 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use crate::{devices::Devices, is_compatible};
use alloc::alloc::{alloc_zeroed, dealloc, handle_alloc_error};
use core::{alloc::Layout, mem::size_of, ptr::NonNull};
use flat_device_tree::Fdt;
use log::{debug, error, info, warn};
use virtio_drivers::{
    BufferDirection, Hal, PAGE_SIZE, PhysAddr,
    device::{
        blk::VirtIOBlk,
        console::VirtIOConsole,
        socket::{VirtIOSocket, VsockConnectionManager},
    },
    transport::{
        DeviceType, DeviceTypeError, SomeTransport, Transport,
        mmio::{MmioError, MmioTransport, VirtIOHeader},
        pci::{
            PciTransport,
            bus::{MmioCam, PciRoot},
            virtio_device_type,
        },
    },
};

const VIRTIO_MMIO_COMPATIBLE: &str = "virtio,mmio";

/// # Safety
///
/// Any VirtIO MMIO devices in the given device tree must exist and be mapped appropriately, and
/// must not be constructed anywhere else.
pub unsafe fn find_virtio_mmio_devices(fdt: &Fdt, devices: &mut Devices) {
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
                    // SAFETY: The caller promised that the device tree is correct, VirtIO MMIO
                    // devices are mapped, and no aliases are constructed to the MMIO region.
                    match unsafe { MmioTransport::new(header, region_size) } {
                        Err(MmioError::InvalidDeviceID(DeviceTypeError::InvalidDeviceType(0))) => {
                            debug!("Ignoring VirtIO device with zero device ID.");
                        }
                        Err(e) => {
                            error!("Error creating VirtIO transport: {e}");
                        }
                        Ok(mut transport) => {
                            info!(
                                "Detected virtio MMIO device with device type {:?}, vendor ID {:#x}, version {:?}, features {:#018x}",
                                transport.device_type(),
                                transport.vendor_id(),
                                transport.version(),
                                transport.read_device_features(),
                            );
                            init_virtio_device(transport.into(), devices);
                        }
                    }
                }
            } else {
                error!("VirtIO MMIO device {} missing region", node.name);
            }
        }
    }
}

fn init_virtio_device(transport: SomeTransport<'static>, devices: &mut Devices) {
    match transport.device_type() {
        DeviceType::Block => {
            devices.block.push(VirtIOBlk::new(transport).unwrap());
        }
        DeviceType::Console => {
            devices.console.push(VirtIOConsole::new(transport).unwrap());
        }
        DeviceType::Socket => {
            devices.vsock.push(VsockConnectionManager::new(
                VirtIOSocket::new(transport).unwrap(),
            ));
        }
        t => {
            warn!("Ignoring unsupported VirtIO device type {t:?}");
        }
    }
}

pub fn find_virtio_pci_devices(pci_root: &mut PciRoot<MmioCam>, devices: &mut Devices) {
    info!("Looking for VirtIO devices on PCI bus");
    for (device_function, info) in pci_root.enumerate_bus(0) {
        if let Some(virtio_type) = virtio_device_type(&info) {
            info!("  VirtIO {virtio_type:?} {info} at {device_function}");
            let mut transport =
                PciTransport::new::<VirtioHal, _>(pci_root, device_function).unwrap();
            info!(
                "Detected virtio PCI device with device type {:?}, features {:#018x}, status {:?}",
                transport.device_type(),
                transport.read_device_features(),
                transport.get_status(),
            );
            init_virtio_device(transport.into(), devices);
        }
    }
}

#[derive(Debug)]
pub struct VirtioHal;

// SAFETY: dma_alloc and mmio_phys_to_virt always return appropriate pointers based on their
// parameters.
unsafe impl Hal for VirtioHal {
    fn dma_alloc(pages: usize, _direction: BufferDirection) -> (PhysAddr, NonNull<u8>) {
        assert_ne!(pages, 0);
        let layout = Layout::from_size_align(pages * PAGE_SIZE, PAGE_SIZE).unwrap();
        // SAFETY: The layout has a non-zero size because we just checked that `pages` is non-zero.
        let vaddr = unsafe { alloc_zeroed(layout) };
        let vaddr = if let Some(vaddr) = NonNull::new(vaddr) {
            vaddr
        } else {
            handle_alloc_error(layout)
        };
        let paddr = virt_to_phys(vaddr.as_ptr() as _);
        (paddr, vaddr)
    }

    unsafe fn dma_dealloc(_paddr: PhysAddr, vaddr: NonNull<u8>, pages: usize) -> i32 {
        let layout = Layout::from_size_align(pages * PAGE_SIZE, PAGE_SIZE).unwrap();
        // SAFETY: the memory was allocated by `dma_alloc` above using the same allocator, and the
        // layout is the same as was used then.
        unsafe {
            dealloc(vaddr.as_ptr(), layout);
        }
        0
    }

    unsafe fn mmio_phys_to_virt(paddr: PhysAddr, _size: usize) -> NonNull<u8> {
        NonNull::new(paddr as _).unwrap()
    }

    unsafe fn share(buffer: NonNull<[u8]>, _direction: BufferDirection) -> PhysAddr {
        let vaddr = buffer.as_ptr() as *mut u8 as usize;
        // Nothing to do, as the host already has access to all memory.
        virt_to_phys(vaddr)
    }

    unsafe fn unshare(_paddr: PhysAddr, _buffer: NonNull<[u8]>, _direction: BufferDirection) {
        // Nothing to do, as the host already has access to all memory and we didn't copy the buffer
        // anywhere else.
    }
}

fn virt_to_phys(vaddr: usize) -> PhysAddr {
    vaddr as _
}
