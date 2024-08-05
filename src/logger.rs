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

use crate::console::SharedConsole;
use embedded_io::Write;
use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};

impl<T: Send + 'static> Log for SharedConsole<T>
where
    for<'a> &'a SharedConsole<T>: Write,
{
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(mut self: &Self, record: &Record) {
        writeln!(&mut self, "[{}] {}", record.level(), record.args()).unwrap();
    }

    fn flush(&self) {}
}

/// Initialises the logger with the given shared console.
pub fn init(console: &'static impl Log, max_level: LevelFilter) -> Result<(), SetLoggerError> {
    log::set_logger(console)?;
    log::set_max_level(max_level);
    Ok(())
}
