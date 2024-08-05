use crate::{
    apps::alarm,
    devices::Devices,
    exceptions::set_irq_handler,
    platform::{Platform, PlatformImpl},
};
use arm_gic::{
    gicv3::{GicV3, IntId},
    irq_enable,
};
use arm_pl031::Rtc;
use arrayvec::ArrayVec;
use core::str;
use embedded_io::{Read, Write};
use log::info;
use virtio_drivers::{
    device::socket::{DisconnectReason, VsockAddr, VsockConnectionManager, VsockEventType},
    transport::{
        pci::{bus::PciRoot, virtio_device_type},
        Transport,
    },
    Hal,
};

const EOF: u8 = 0x04;

pub fn main(
    console: &mut (impl Write + Read),
    gic: &mut GicV3,
    pci_roots: &mut [PciRoot],
    devices: &mut Devices,
) {
    info!("Configuring IRQs...");
    GicV3::set_priority_mask(0xff);
    alarm::irq_setup(gic);
    set_irq_handler(Some(&irq_handler));
    irq_enable();

    loop {
        write!(console, "$ ").unwrap();
        let line = read_line(console);
        if line.as_ref() == [EOF] {
            break;
        }
        let Ok(line) = str::from_utf8(&line) else {
            writeln!(console, "Invalid UTF-8").unwrap();
            continue;
        };
        let mut parts = line.split(' ');
        let Some(command) = parts.next() else {
            continue;
        };
        match command {
            "alarm" => alarm::alarm(console, parts, &mut devices.rtc),
            "date" => date(console, &mut devices.rtc),
            "exit" => break,
            "help" => help(console),
            "lsdev" => lsdev(console, devices),
            "lspci" => lspci(console, pci_roots),
            "vcat" => vcat(console, parts, &mut devices.vsock),
            _ => {
                writeln!(console, "Unrecognised command.").unwrap();
            }
        }
    }
    set_irq_handler(None);
}

fn read_line(console: &mut (impl Write + Read)) -> ArrayVec<u8, 128> {
    let mut line: ArrayVec<u8, 128> = ArrayVec::new();
    loop {
        let mut c = [0];
        console.read_exact(&mut c).unwrap();
        match c[0] {
            b'\r' | b'\n' => {
                console.write_all(b"\r\n").unwrap();
                return line;
            }
            EOF if line.is_empty() => {
                console.write_all(b"\r\n").unwrap();
                line.push(EOF);
                return line;
            }
            c => {
                if !c.is_ascii_control() {
                    console.write_all(&[c]).unwrap();
                    line.push(c);
                }
            }
        }
    }
}

fn irq_handler(intid: IntId) {
    match intid {
        PlatformImpl::RTC_IRQ => {
            alarm::irq_handle();
        }
        _ => {
            panic!("Unexpected IRQ {:?}", intid);
        }
    }
}

fn date(console: &mut (impl Write + Read), rtc: &mut Rtc) {
    let time = rtc.get_time();
    writeln!(console, "{}", time).unwrap();
}

fn help(console: &mut (impl Write + Read)) {
    writeln!(console, "Commands:").unwrap();
    writeln!(
        console,
        "  alarm - Sets an alarm for 5 seconds in the future"
    )
    .unwrap();
    writeln!(console, "  date - Prints the current date and time").unwrap();
    writeln!(
        console,
        "  exit - Exits the shell and powers off the system"
    )
    .unwrap();
    writeln!(console, "  help - Prints this help").unwrap();
    writeln!(console, "  lsdev - Lists devices").unwrap();
    writeln!(console, "  lspci - Lists devices on the PCI bus").unwrap();
    writeln!(console, "  vcat - Communicates with a vsock port").unwrap();
}

fn lsdev(console: &mut impl Write, devices: &mut Devices) {
    writeln!(console, "Block devices:").unwrap();
    for (i, device) in devices.block.iter_mut().enumerate() {
        let mut id_buffer = [0; 20];
        let id_len = match device.device_id(&mut id_buffer) {
            Ok(id_len) => id_len,
            Err(e) => {
                writeln!(console, "Error getting ID: {}", e).unwrap();
                0
            }
        };
        let id = str::from_utf8(&id_buffer[..id_len]).unwrap();
        writeln!(
            console,
            "  {}: \"{}\", capacity {} sectors, {}",
            i,
            id,
            device.capacity(),
            if device.readonly() {
                "read-only"
            } else {
                "read-write"
            }
        )
        .unwrap();
    }
    writeln!(console, "Console devices:").unwrap();
    for (i, device) in devices.console.iter_mut().enumerate() {
        writeln!(console, "  {}: {:?}", i, device.info()).unwrap();
    }
    writeln!(console, "Vsock devices:").unwrap();
    for (i, device) in devices.vsock.iter_mut().enumerate() {
        writeln!(console, "  {}: guest CID {}", i, device.guest_cid()).unwrap();
    }
}

fn lspci(console: &mut impl Write, pci_roots: &mut [PciRoot]) {
    writeln!(console, "{} PCI roots", pci_roots.len()).unwrap();
    for pci_root in pci_roots {
        for (device_function, info) in pci_root.enumerate_bus(0) {
            let (status, command) = pci_root.get_status_command(device_function);
            writeln!(
                console,
                "{} at {}, status {:?} command {:?}",
                info, device_function, status, command
            )
            .unwrap();
            if let Some(virtio_type) = virtio_device_type(&info) {
                writeln!(console, "  VirtIO {:?}", virtio_type).unwrap();
            }
            for (bar_index, info) in pci_root
                .bars(device_function)
                .unwrap()
                .into_iter()
                .enumerate()
            {
                if let Some(info) = info {
                    writeln!(console, "  BAR {}: {}", bar_index, info).unwrap();
                }
            }
        }
    }
}

fn vcat<'a, H: Hal, T: Transport>(
    console: &mut (impl Write + Read),
    args: impl Iterator<Item = &'a str>,
    vsock: &mut [VsockConnectionManager<H, T>],
) {
    let args = args.collect::<ArrayVec<_, 4>>();
    if args.len() != 2 {
        writeln!(console, "Usage:").unwrap();
        writeln!(console, "  vcat <CID> <port>").unwrap();
        return;
    }
    let Ok(cid) = args[0].parse() else {
        writeln!(console, "Invalid CID {}", args[0]).unwrap();
        return;
    };
    let Ok(port) = args[1].parse() else {
        writeln!(console, "Invalid port {}", args[1]).unwrap();
        return;
    };
    let Some(vsock) = vsock.get_mut(0) else {
        writeln!(console, "No vsock device found.").unwrap();
        return;
    };
    let local_port = 42;
    let peer = VsockAddr { cid, port };
    writeln!(console, "Connecting to {}:{}...", peer.cid, peer.port).unwrap();
    vsock.connect(peer, local_port).unwrap();

    loop {
        if let Some(event) = vsock.poll().unwrap() {
            if event.destination.port == local_port && event.source == peer {
                match event.event_type {
                    VsockEventType::Connected => {
                        writeln!(console, "Connected.").unwrap();
                    }
                    VsockEventType::Disconnected {
                        reason: DisconnectReason::Shutdown,
                    } => {
                        writeln!(console, "Connection shut down.").unwrap();
                        return;
                    }
                    VsockEventType::Disconnected {
                        reason: DisconnectReason::Reset,
                    } => {
                        writeln!(console, "Connection reset.").unwrap();
                        return;
                    }
                    VsockEventType::Received { .. } => {
                        while vsock.recv_buffer_available_bytes(peer, local_port).unwrap() > 0 {
                            let mut recv_buffer = [0; 10];
                            let bytes_read =
                                vsock.recv(peer, local_port, &mut recv_buffer).unwrap();
                            console.write_all(&recv_buffer[0..bytes_read]).unwrap();
                        }
                    }
                    VsockEventType::CreditUpdate => {}
                    _ => {
                        writeln!(console, "Event: {:?}", event).unwrap();
                    }
                }
            } else {
                writeln!(
                    console,
                    "Event for unexpected source or destination: {:?}",
                    event
                )
                .unwrap();
            }
        }
    }
}
