// Copyright 2024 Google LLC.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use crate::console::SharedConsole;
use embedded_io::Write;
use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};
use percore::exception_free;

impl<T: Send + Write> Log for SharedConsole<T> {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        exception_free(|token| {
            let console = &mut *self.console.borrow(token).lock();
            writeln!(console, "[{}] {}", record.level(), record.args()).unwrap();
        });
    }

    fn flush(&self) {}
}

/// Initialises the logger with the given shared console.
pub fn init(console: &'static impl Log, max_level: LevelFilter) -> Result<(), SetLoggerError> {
    log::set_logger(console)?;
    log::set_max_level(max_level);
    Ok(())
}
