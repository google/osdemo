use crate::{
    apps::alarm,
    exceptions::set_irq_handler,
    platform::{Platform, PlatformImpl},
};
use arm_gic::{
    gicv3::{GicV3, IntId},
    irq_enable,
};
use arm_pl031::Rtc;
use chrono::{TimeZone, Utc};
use core::fmt::Write;
use embedded_io::Read;
use log::info;
use tinyvec::{array_vec, ArrayVec};

const EOF: u8 = 0x04;

pub fn main(console: &mut (impl Write + Read), rtc: &mut Rtc, gic: &mut GicV3) {
    info!("Configuring IRQs...");
    GicV3::set_priority_mask(0xff);
    alarm::irq_setup(gic);
    set_irq_handler(Some(&irq_handler));
    irq_enable();

    loop {
        alarm::irq_finish(rtc);

        write!(console, "$ ").unwrap();
        let line = read_line(console);
        match line.as_ref() {
            b"" => {}
            b"alarm" => alarm::alarm(console, rtc),
            b"date" => date(console, rtc),
            b"exit" | [EOF] => break,
            b"help" => help(console),
            _ => {
                writeln!(console, "Unrecognised command.").unwrap();
            }
        }
    }
    set_irq_handler(None);
}

fn read_line(console: &mut (impl Write + Read)) -> ArrayVec<[u8; 128]> {
    let mut line: ArrayVec<[u8; 128]> = array_vec![];
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
    let timestamp = rtc.get_unix_timestamp();
    let time = Utc.timestamp_opt(timestamp.into(), 0).unwrap();
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
}
