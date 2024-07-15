use core::fmt::Write;
use embedded_io::Read;

pub fn main(console: &mut (impl Write + Read)) {
    writeln!(console, "Shell starting").unwrap();
}
