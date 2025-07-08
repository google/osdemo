// Copyright 2024 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use crate::platform::{ConsoleImpl, Platform, PlatformImpl};
use arm_gic::{IntId, wfi};
use core::panic::PanicInfo;
use embedded_io::{ErrorType, Read, ReadReady, Write};
use percore::{ExceptionLock, exception_free};
use spin::{Once, mutex::SpinMutex};

static CONSOLE: Once<SharedConsole<ConsoleImpl>> = Once::new();

/// A console guarded by a spin mutex so that it may be shared between threads.
///
/// Any thread may write to it, but only a single thread may read from it.
pub struct SharedConsole<T: Send> {
    pub console: ExceptionLock<SpinMutex<T>>,
}

impl<T: ErrorType + Send> ErrorType for &SharedConsole<T> {
    type Error = T::Error;
}

impl<T: ErrorType + Send + Write> Write for &SharedConsole<T> {
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

impl<T: ErrorType + Send + Write> Write for Console<T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.shared.write(buf)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.shared.flush()
    }
}

impl<T: ErrorType + Send + 'static> ErrorType for Console<T> {
    type Error = T::Error;
}

impl<T: ErrorType + InterruptRead + Read + ReadReady + Send + 'static> Read for Console<T> {
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
            T::wait_for_irq();
        }
    }
}

impl<T: ErrorType + ReadReady + Send + 'static> ReadReady for Console<T> {
    fn read_ready(&mut self) -> Result<bool, Self::Error> {
        exception_free(|token| self.shared.console.borrow(token).lock().read_ready())
    }
}

impl<T: Send + InterruptRead> Console<T> {
    /// Lets the underlying UART driver handle the given interrupt.
    pub fn handle_irq(intid: IntId) {
        let console = CONSOLE.get().unwrap();
        exception_free(|token| {
            console.console.borrow(token).lock().handle_irq(intid);
        });
    }
}

/// Trait to read characters from a UART in an interrupt-driven way.
pub trait InterruptRead {
    /// Waits for an IRQ. May return early.
    fn wait_for_irq() {
        wfi();
    }

    /// Handles the given interrupt for the UART.
    ///
    /// Note that this is called with the console locked, so must not try to log anything.
    fn handle_irq(&mut self, intid: IntId);
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
