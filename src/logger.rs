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
use core::fmt::Write;
use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};
use spin::mutex::SpinMutex;

static LOGGER: Logger<Console> = Logger {
    console: SpinMutex::new(None),
};

struct Logger<T: Send + Write> {
    console: SpinMutex<Option<T>>,
}

impl<T: Send + Write> Log for Logger<T> {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        writeln!(
            self.console.lock().as_mut().unwrap(),
            "[{}] {}",
            record.level(),
            record.args()
        )
        .unwrap();
    }

    fn flush(&self) {}
}

/// Initialises console logger.
pub fn init(console: Console, max_level: LevelFilter) -> Result<(), SetLoggerError> {
    LOGGER.console.lock().replace(console);

    log::set_logger(&LOGGER)?;
    log::set_max_level(max_level);
    Ok(())
}
