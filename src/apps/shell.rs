use core::fmt::Write;
use embedded_io::Read;
use tinyvec::{array_vec, ArrayVec};

pub fn main(console: &mut (impl Write + Read)) {
    loop {
        write!(console, "$ ").unwrap();
        let line = read_line(console);
        match line.as_ref() {
            b"exit" => break,
            _ => {
                writeln!(console, "Unrecognised command.").unwrap();
            }
        }
    }
}

fn read_line(console: &mut (impl Write + Read)) -> ArrayVec<[u8; 128]> {
    let mut line: ArrayVec<[u8; 128]> = array_vec![];
    loop {
        let mut c = [0];
        console.read_exact(&mut c).unwrap();
        let c = c[0];
        if c == b'\r' {
            console.write_str("\r\n").unwrap();
            return line;
        } else {
            console.write_char(c.into()).unwrap();
            line.push(c);
        }
    }
}
