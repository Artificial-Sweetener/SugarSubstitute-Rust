// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe actual tracing output through an isolated external writer.
use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
};

#[derive(Clone, Default)]
pub(super) struct Output(Arc<Mutex<Vec<u8>>>);
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Output {
    /// Install a thread-local subscriber; concurrent tests retain independent sinks.
    pub(super) fn observe(&self) -> tracing::subscriber::DefaultGuard {
        let output = self.clone();
        tracing::subscriber::set_default(
            tracing_subscriber::fmt()
                .without_time()
                .with_ansi(false)
                .with_writer(move || output.clone())
                .finish(),
        )
    }
    /// Borrow no runtime cause or custody while inspecting the external diagnostic bytes.
    pub(super) fn text(&self) -> Result<String, std::string::FromUtf8Error> {
        String::from_utf8(self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone())
    }
}
