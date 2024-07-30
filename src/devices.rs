use crate::virtio::VirtioHal;
use alloc::vec::Vec;
use virtio_drivers::{
    device::{blk::VirtIOBlk, console::VirtIOConsole, socket::VsockConnectionManager},
    transport::mmio::MmioTransport,
};

#[derive(Default)]
pub struct Devices {
    pub block: Vec<VirtIOBlk<VirtioHal, MmioTransport>>,
    pub console: Vec<VirtIOConsole<VirtioHal, MmioTransport>>,
    pub vsock: Vec<VsockConnectionManager<VirtioHal, MmioTransport>>,
}
