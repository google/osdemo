use crate::platform::{ConsoleImpl, Platform, PlatformImpl};
use core::{convert::Infallible, fmt, panic::PanicInfo};
use embedded_io::{ErrorType, Read, Write};
use spin::{mutex::SpinMutex, Once};

static CONSOLE: Once<SharedConsole<ConsoleImpl>> = Once::new();

/// A console guarded by a spin mutex so that it may be shared between threads.
pub struct SharedConsole<T: Send> {
    console: SpinMutex<T>,
}

impl<T: Send> ErrorType for &SharedConsole<T> {
    type Error = Infallible;
}

impl<T: Send + fmt::Write> fmt::Write for &SharedConsole<T> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.console.lock().write_str(s)
    }
}

impl<T: Send + ErrorType<Error = Self::Error> + Write> Write for &SharedConsole<T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.console.lock().write(buf)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.console.lock().flush()
    }
}

impl<T: Send + ErrorType<Error = Self::Error> + Read> Read for &SharedConsole<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.console.lock().read(buf)
    }
}

/// Initialises the shared console.
pub fn init(console: ConsoleImpl) -> &'static SharedConsole<ConsoleImpl> {
    let shared = CONSOLE.call_once(|| SharedConsole {
        console: SpinMutex::new(console),
    });

    shared
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(console) = CONSOLE.get() {
        // Ignore any errors writing to the console, to avoid panicking recursively.
        let _ = writeln!(console.console.lock(), "{}", info);
    }
    PlatformImpl::power_off();
}
