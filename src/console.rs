use crate::platform::{ConsoleImpl, Platform, PlatformImpl};
use core::{convert::Infallible, panic::PanicInfo};
use embedded_io::{ErrorType, Read, ReadReady, Write};
use percore::{exception_free, ExceptionLock};
use spin::{mutex::SpinMutex, Once};

static CONSOLE: Once<SharedConsole<ConsoleImpl>> = Once::new();

/// A console guarded by a spin mutex so that it may be shared between threads.
///
/// Any thread may write to it, but only a single thread may read from it.
pub struct SharedConsole<T: Send> {
    console: ExceptionLock<SpinMutex<T>>,
}

impl<T: Send> ErrorType for &SharedConsole<T> {
    type Error = Infallible;
}

impl<T: Send + ErrorType<Error = Self::Error> + Write> Write for &SharedConsole<T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        exception_free(|token| self.console.borrow(token).lock().write(buf))
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        exception_free(|token| self.console.borrow(token).lock().flush())
    }
}

/// The owner of a shared console, who has unique read access.
///
/// The reading side can't be shared, as the caller of `ReadReady::read_ready` needs to be
/// guaranteed that bytes will be available to read when the next call `Read::read`.
pub struct Console<T: Send + 'static> {
    shared: &'static SharedConsole<T>,
}

impl<T: Send + 'static> Console<T> {
    /// Returns a shared writer for the console. This may be copied freely.
    pub fn shared(&self) -> &'static SharedConsole<T> {
        self.shared
    }
}

impl<T: Send + ErrorType<Error = Self::Error> + Write> Write for Console<T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.shared.write(buf)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.shared.flush()
    }
}

impl<T: Send + 'static> ErrorType for Console<T> {
    type Error = Infallible;
}

impl<T: Send + ErrorType<Error = Self::Error> + Read + ReadReady + 'static> Read for Console<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        // Wait until the console has some data to read, without holding the lock and keeping
        // exceptions masked the whole time.
        loop {
            if let Some(result) = exception_free(|token| {
                let mut console = self.shared.console.borrow(token).lock();
                match console.read_ready()? {
                    true => Ok::<_, Self::Error>(Some(console.read(buf)?)),
                    false => Ok(None),
                }
            })? {
                break Ok(result);
            }
        }
    }
}

impl<T: Send + ErrorType<Error = Self::Error> + ReadReady + 'static> ReadReady for Console<T> {
    fn read_ready(&mut self) -> Result<bool, Self::Error> {
        exception_free(|token| self.shared.console.borrow(token).lock().read_ready())
    }
}

/// Initialises the shared console.
pub fn init(console: ConsoleImpl) -> Console<ConsoleImpl> {
    let shared = CONSOLE.call_once(|| SharedConsole {
        console: ExceptionLock::new(SpinMutex::new(console)),
    });
    Console { shared }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(console) = CONSOLE.get() {
        exception_free(|token| {
            // Ignore any errors writing to the console, to avoid panicking recursively.
            let _ = writeln!(console.console.borrow(token).lock(), "{}", info);
        });
    }
    PlatformImpl::power_off();
}
