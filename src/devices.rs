use crate::virtio::VirtioHal;
use alloc::vec::Vec;
use virtio_drivers::{device::blk::VirtIOBlk, transport::mmio::MmioTransport};

#[derive(Default)]
pub struct Devices {
    pub block: Vec<VirtIOBlk<VirtioHal, MmioTransport>>,
}
