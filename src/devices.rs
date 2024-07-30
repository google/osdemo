use crate::virtio::{SomeTransport, VirtioHal};
use alloc::vec::Vec;
use virtio_drivers::device::{
    blk::VirtIOBlk, console::VirtIOConsole, socket::VsockConnectionManager,
};

#[derive(Default)]
pub struct Devices {
    pub block: Vec<VirtIOBlk<VirtioHal, SomeTransport>>,
    pub console: Vec<VirtIOConsole<VirtioHal, SomeTransport>>,
    pub vsock: Vec<VsockConnectionManager<VirtioHal, SomeTransport>>,
}
