// Copyright 2024 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

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
