use crate::{devices::Devices, is_compatible};
use alloc::alloc::{alloc_zeroed, dealloc, handle_alloc_error};
use core::{alloc::Layout, mem::size_of, ptr::NonNull};
use flat_device_tree::Fdt;
use log::{debug, error, info, warn};
use virtio_drivers::{
    device::blk::VirtIOBlk,
    transport::{
        mmio::{MmioError, MmioTransport, VirtIOHeader},
        DeviceType, Transport,
    },
    BufferDirection, Hal, PhysAddr, PAGE_SIZE,
};

const VIRTIO_MMIO_COMPATIBLE: &str = "virtio,mmio";

pub fn find_virtio_mmio_devices(fdt: &Fdt, devices: &mut Devices) {
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
                            init_virtio_device(transport, devices);
                        }
                    }
                }
            } else {
                error!("VirtIO MMIO device {} missing region", node.name);
            }
        }
    }
}

fn init_virtio_device(transport: MmioTransport, devices: &mut Devices) {
    match transport.device_type() {
        DeviceType::Block => {
            let block = VirtIOBlk::new(transport).unwrap();
            devices.block.push(block);
        }
        t => {
            warn!("Ignoring unsupported VirtIO device type {:?}", t);
        }
    }
}

#[derive(Debug)]
pub struct VirtioHal;

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
    vaddr
}
