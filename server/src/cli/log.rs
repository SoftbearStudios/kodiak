// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: LGPL-3.0-or-later

use super::Options;
use crate::{LogLevel, NonZeroUnixMillis, ServerLogDto, UnixTime};
use kodiak_common::rand::{thread_rng, Rng};
use log::{Level, LevelFilter, Log};
use std::cell::RefCell;
use std::fmt::Write as _;
use std::io::{stderr, Write as _};
use std::sync::Mutex;

impl Options {
    pub(crate) fn init_logger(&self) {
        log::set_boxed_logger(Box::new(Logger {
            game: self.debug_game,
            engine: self.debug_engine,
            plasma: self.debug_plasma,
            http: self.debug_http,
        }))
        .expect("failed to init logger");
        log::set_max_level(
            self.debug_game
                .max(self.debug_engine)
                .max(self.debug_plasma)
                .max(self.debug_http),
        );
    }
}

struct Logger {
    game: LevelFilter,
    engine: LevelFilter,
    plasma: LevelFilter,
    http: LevelFilter,
}

impl Logger {
    fn filter(&self, target: &str) -> LevelFilter {
        if target.starts_with("kodiak_server::entry_point")
            || target.starts_with("kodiak_server::router")
        {
            self.http
        } else if target.starts_with("kodiak_server::plasma")
            || target.starts_with("kodiak_server::net::web_socket")
        {
            self.plasma
        } else if target.starts_with("server") || target.starts_with("common") {
            self.game
        } else {
            self.engine
        }
    }
}

impl Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        if matches!(
            metadata.target(),
            // Illegal SNI hostname received "a.b.c.d"
            "rustls::msgs::handshake"
        ) {
            false
        } else {
            self.filter(metadata.target()) >= metadata.level()
        }
    }

    fn flush(&self) {
        // No-op
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        thread_local! {
            static BUFFER: RefCell<String> = const { RefCell::new(String::new()) };
        }
        BUFFER.with(|buf| {
            let mut buf = buf.borrow_mut();
            buf.clear();
            writeln!(
                &mut *buf,
                "[{} {}] {}",
                record.level(),
                record.target(),
                record.args()
            )
            .unwrap();
            let result = stderr().lock().write_all(buf.as_bytes());
            if cfg!(debug_assertions) {
                result.unwrap();
            }
        });

        if !match record.level() {
            Level::Error => true,
            Level::Warn => thread_rng().gen_bool(0.1),
            Level::Info => thread_rng().gen_bool(0.01),
            // Don't send debug/trace to plasma.
            _ => false,
        } {
            return;
        }
        let level = match record.level() {
            Level::Error => LogLevel::Error,
            Level::Warn => LogLevel::Warn,
            Level::Info => LogLevel::Info,
            Level::Debug => LogLevel::Debug,
            Level::Trace => LogLevel::Trace,
        };
        let source = record.target().to_owned();
        let message = record.args().to_string();
        let mut timestamp = NonZeroUnixMillis::now();

        let mut logs = LOGS.lock().unwrap();
        if logs.len() < 1024 {
            if let Some(last) = logs.last() {
                // Hack: Unique time.
                timestamp = timestamp.max(last.timestamp.add_millis(1));
            }
            logs.push(ServerLogDto {
                timestamp: timestamp.into(),
                level,
                source,
                message,
            });
        }
    }
}

pub static LOGS: Mutex<Vec<ServerLogDto>> = Mutex::new(Vec::new());
