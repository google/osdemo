use crate::{devices::Devices, is_compatible};
use alloc::alloc::{alloc_zeroed, dealloc, handle_alloc_error};
use core::{alloc::Layout, mem::size_of, ptr::NonNull};
use flat_device_tree::Fdt;
use log::{debug, error, info, warn};
use virtio_drivers::{
    device::{
        blk::VirtIOBlk,
        console::VirtIOConsole,
        socket::{VirtIOSocket, VsockConnectionManager},
    },
    transport::{
        mmio::{MmioError, MmioTransport, VirtIOHeader},
        pci::{bus::PciRoot, virtio_device_type, PciTransport},
        DeviceStatus, DeviceType, Transport,
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

fn init_virtio_device(transport: SomeTransport, devices: &mut Devices) {
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
            warn!("Ignoring unsupported VirtIO device type {:?}", t);
        }
    }
}

pub fn find_virtio_pci_devices(pci_root: &mut PciRoot, devices: &mut Devices) {
    info!("Looking for VirtIO devices on PCI bus");
    for (device_function, info) in pci_root.enumerate_bus(0) {
        if let Some(virtio_type) = virtio_device_type(&info) {
            info!("  VirtIO {:?} {} at {}", virtio_type, info, device_function);
            let mut transport = PciTransport::new::<VirtioHal>(pci_root, device_function).unwrap();
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

/// A wrapper for an arbitrary VirtIO transport, either MMIO or PCI.
#[derive(Debug)]
pub enum SomeTransport {
    Mmio(MmioTransport),
    Pci(PciTransport),
}

impl From<MmioTransport> for SomeTransport {
    fn from(mmio: MmioTransport) -> Self {
        Self::Mmio(mmio)
    }
}

impl From<PciTransport> for SomeTransport {
    fn from(pci: PciTransport) -> Self {
        Self::Pci(pci)
    }
}

impl Transport for SomeTransport {
    fn device_type(&self) -> DeviceType {
        match self {
            Self::Mmio(mmio) => mmio.device_type(),
            Self::Pci(pci) => pci.device_type(),
        }
    }

    fn read_device_features(&mut self) -> u64 {
        match self {
            Self::Mmio(mmio) => mmio.read_device_features(),
            Self::Pci(pci) => pci.read_device_features(),
        }
    }

    fn write_driver_features(&mut self, driver_features: u64) {
        match self {
            Self::Mmio(mmio) => mmio.write_driver_features(driver_features),
            Self::Pci(pci) => pci.write_driver_features(driver_features),
        }
    }

    fn max_queue_size(&mut self, queue: u16) -> u32 {
        match self {
            Self::Mmio(mmio) => mmio.max_queue_size(queue),
            Self::Pci(pci) => pci.max_queue_size(queue),
        }
    }

    fn notify(&mut self, queue: u16) {
        match self {
            Self::Mmio(mmio) => mmio.notify(queue),
            Self::Pci(pci) => pci.notify(queue),
        }
    }

    fn get_status(&self) -> DeviceStatus {
        match self {
            Self::Mmio(mmio) => mmio.get_status(),
            Self::Pci(pci) => pci.get_status(),
        }
    }

    fn set_status(&mut self, status: DeviceStatus) {
        match self {
            Self::Mmio(mmio) => mmio.set_status(status),
            Self::Pci(pci) => pci.set_status(status),
        }
    }

    fn set_guest_page_size(&mut self, guest_page_size: u32) {
        match self {
            Self::Mmio(mmio) => mmio.set_guest_page_size(guest_page_size),
            Self::Pci(pci) => pci.set_guest_page_size(guest_page_size),
        }
    }

    fn requires_legacy_layout(&self) -> bool {
        match self {
            Self::Mmio(mmio) => mmio.requires_legacy_layout(),
            Self::Pci(pci) => pci.requires_legacy_layout(),
        }
    }

    fn queue_set(
        &mut self,
        queue: u16,
        size: u32,
        descriptors: PhysAddr,
        driver_area: PhysAddr,
        device_area: PhysAddr,
    ) {
        match self {
            Self::Mmio(mmio) => mmio.queue_set(queue, size, descriptors, driver_area, device_area),
            Self::Pci(pci) => pci.queue_set(queue, size, descriptors, driver_area, device_area),
        }
    }

    fn queue_unset(&mut self, queue: u16) {
        match self {
            Self::Mmio(mmio) => mmio.queue_unset(queue),
            Self::Pci(pci) => pci.queue_unset(queue),
        }
    }

    fn queue_used(&mut self, queue: u16) -> bool {
        match self {
            Self::Mmio(mmio) => mmio.queue_used(queue),
            Self::Pci(pci) => pci.queue_used(queue),
        }
    }

    fn ack_interrupt(&mut self) -> bool {
        match self {
            Self::Mmio(mmio) => mmio.ack_interrupt(),
            Self::Pci(pci) => pci.ack_interrupt(),
        }
    }

    fn config_space<T: 'static>(&self) -> virtio_drivers::Result<NonNull<T>> {
        match self {
            Self::Mmio(mmio) => mmio.config_space(),
            Self::Pci(pci) => pci.config_space(),
        }
    }
}
