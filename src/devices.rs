// Copyright 2024 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use crate::virtio::VirtioHal;
use alloc::vec::Vec;
use arm_pl031::Rtc;
use virtio_drivers::{
    device::{blk::VirtIOBlk, console::VirtIOConsole, socket::VsockConnectionManager},
    transport::SomeTransport,
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
