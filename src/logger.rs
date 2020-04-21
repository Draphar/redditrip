/*
 * Copyright 2020 Joshua Prieth
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

/*!
A logging implementation for this crate.
*/

use std::{fmt::Display, process};

use ansi_term::Color;
use log::{Level, LevelFilter, Log, Metadata, Record};

/// A logging implementation for this crate.
struct Logger {
    /// Whether colors should be sent to stdout.
    stdout_colors: bool,

    /// Whether colors should be sent to stderr.
    stderr_colors: bool,
}

impl Log for Logger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if !record.target().starts_with("redditrip") {
            return;
        };

        match record.level() {
            Level::Trace => {
                println!("[TRACE]   {}:{}", record.target(), record.args());
            }
            Level::Debug => {
                println!("[VERBOSE] {}", record.args());
            }
            Level::Info => {
                let text: Box<dyn Display> = if cfg!(not(windows)) && self.stdout_colors {
                    Box::new(Color::Green.paint("[INFO]"))
                } else {
                    Box::new("[INFO]")
                };
                println!("{}    {}", text, record.args());
            }
            Level::Warn => {
                let text: Box<dyn Display> = if cfg!(not(windows)) && self.stderr_colors {
                    Box::new(Color::Red.paint("[ERROR]"))
                } else {
                    Box::new("[ERROR]")
                };
                eprintln!("{}   {}", text, record.args());
            }
            Level::Error => {
                let text: Box<dyn Display> = if cfg!(not(windows)) && self.stderr_colors {
                    Box::new(Color::Red.bold().italic().paint("[FATAL]   Fatal error"))
                } else {
                    Box::new("[FATAL]   Fatal error")
                };
                eprintln!("{}\n  caused by:\n    {}", text, record.args());
            }
        };
    }

    fn flush(&self) {}
}

/// Initializes the logger.
pub fn init(verbose: usize, stdout_colors: bool, stderr_colors: bool) {
    let logger = Logger {
        stdout_colors,
        stderr_colors,
    };

    match log::set_boxed_logger(Box::new(logger)) {
        Ok(()) => match verbose {
            0 => log::set_max_level(LevelFilter::Off),
            1 => log::set_max_level(LevelFilter::Error),
            2 => log::set_max_level(LevelFilter::Warn),
            3 => log::set_max_level(LevelFilter::Info),
            4 => log::set_max_level(LevelFilter::Debug),
            5 => log::set_max_level(LevelFilter::Trace),
            _ => unreachable!(), // Guaranteed from `main()`
        },
        Err(e) => {
            let text: Box<dyn Display> = if cfg!(not(windows)) && stderr_colors {
                Box::new(Color::Red.bold().italic().paint("[FATAL]   Fatal error"))
            } else {
                Box::new("[FATAL]   Fatal error")
            };
            eprintln!(
                "\n\n{}\n  caused by:\n    Failed to initialize logging system: {}",
                text, e
            );
            process::exit(1);
        }
    };
}
