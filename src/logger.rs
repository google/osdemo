// Copyright 2023 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::platform::Console;
use core::{
    convert::Infallible,
    fmt::{self},
};
use embedded_io::{ErrorType, Read, Write};
use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};
use spin::{
    mutex::{SpinMutex, SpinMutexGuard},
    Once,
};

static LOGGER: Once<Logger<Console>> = Once::new();

struct Logger<T: Send + Write> {
    console: SpinMutex<T>,
}

impl<T: Send + Write> Log for Logger<T> {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        writeln!(
            self.console.lock(),
            "[{}] {}",
            record.level(),
            record.args()
        )
        .unwrap();
    }

    fn flush(&self) {}
}

#[derive(Copy, Clone)]
pub struct SharedConsole {
    logger: &'static Logger<Console>,
}

impl SharedConsole {
    fn lock(&self) -> SpinMutexGuard<Console> {
        self.logger.console.lock()
    }
}

impl ErrorType for SharedConsole {
    type Error = Infallible;
}

impl fmt::Write for SharedConsole {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.lock().write_str(s)
    }
}

impl Write for SharedConsole {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.lock().write(buf)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.lock().flush()
    }
}

impl Read for SharedConsole {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.lock().read(buf)
    }
}

/// Initialises console logger.
pub fn init(console: Console, max_level: LevelFilter) -> Result<SharedConsole, SetLoggerError> {
    let logger = LOGGER.call_once(|| Logger {
        console: SpinMutex::new(console),
    });

    log::set_logger(logger)?;
    log::set_max_level(max_level);
    Ok(SharedConsole { logger })
}
