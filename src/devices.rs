use crate::virtio::{SomeTransport, VirtioHal};
use alloc::vec::Vec;
use arm_pl031::Rtc;
use virtio_drivers::device::{
    blk::VirtIOBlk, console::VirtIOConsole, socket::VsockConnectionManager,
};

pub struct Devices {
    pub rtc: Rtc,
    pub block: Vec<VirtIOBlk<VirtioHal, SomeTransport>>,
    pub console: Vec<VirtIOConsole<VirtioHal, SomeTransport>>,
    pub vsock: Vec<VsockConnectionManager<VirtioHal, SomeTransport>>,
}

impl Devices {
    pub fn new(rtc: Rtc) -> Self {
        Self {
            rtc,
            block: Vec::new(),
            console: Vec::new(),
            vsock: Vec::new(),
        }
    }
}
