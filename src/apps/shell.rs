use crate::{
    apps::alarm,
    devices::Devices,
    exceptions::set_irq_handler,
    pci::PciRoots,
    platform::{Platform, PlatformImpl},
};
use arm_gic::{
    gicv3::{GicV3, IntId},
    irq_enable,
};
use arm_pl031::Rtc;
use arrayvec::ArrayVec;
use core::{fmt::Write, str};
use embedded_io::Read;
use log::info;
use virtio_drivers::transport::pci::virtio_device_type;

const EOF: u8 = 0x04;

pub fn main(
    console: &mut (impl Write + Read),
    rtc: &mut Rtc,
    gic: &mut GicV3,
    pci_roots: &mut PciRoots,
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
        match line.as_ref() {
            b"" => {}
            b"alarm" => alarm::alarm(console, rtc),
            b"date" => date(console, rtc),
            b"exit" | [EOF] => break,
            b"help" => help(console),
            b"lsdev" => lsdev(console, devices),
            b"lspci" => lspci(console, pci_roots),
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
                console.write_str("\r\n").unwrap();
                return line;
            }
            EOF if line.is_empty() => {
                console.write_str("\r\n").unwrap();
                line.push(EOF);
                return line;
            }
            c => {
                if !c.is_ascii_control() {
                    console.write_char(c.into()).unwrap();
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

fn lspci(console: &mut impl Write, pci_roots: &mut PciRoots) {
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
            let mut bar_index = 0;
            while bar_index < 6 {
                let info = pci_root.bar_info(device_function, bar_index).unwrap();
                writeln!(console, "  BAR {}: {}", bar_index, info).unwrap();
                bar_index += 1;
                if info.takes_two_entries() {
                    bar_index += 1;
                }
            }
        }
    }
}
