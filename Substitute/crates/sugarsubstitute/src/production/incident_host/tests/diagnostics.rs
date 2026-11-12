// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe actual tracing output through an isolated external writer.
use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
};

#[derive(Clone, Default)]
pub(in crate::production) struct Output(Arc<Mutex<Vec<u8>>>);
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
/// Retain both registrations until the installed formatter has left this thread.
/// The additional registry is never installed and receives no events or causes.
pub(in crate::production) struct ScopedCapture {
    _subscriber: tracing::subscriber::DefaultGuard,
    _callsite_registry: tracing::Dispatch,
}
impl Output {
    /// Install a thread-local formatter with independent output and stable callsite interest.
    pub(in crate::production) fn observe(&self) -> ScopedCapture {
        // The pinned tracing-core single-registrar path samples the registering
        // thread's default. Two live registrars keep unsubscribed threads from
        // disabling this capture's shared callsites during first registration.
        let callsite_registry = tracing::Dispatch::new(tracing_subscriber::registry());
        let output = self.clone();
        let subscriber = tracing::subscriber::set_default(
            tracing_subscriber::fmt()
                .without_time()
                .with_ansi(false)
                .with_writer(move || output.clone())
                .finish(),
        );
        ScopedCapture { _subscriber: subscriber, _callsite_registry: callsite_registry }
    }
    /// Borrow no runtime cause or custody while inspecting the external diagnostic bytes.
    pub(in crate::production) fn text(&self) -> Result<String, std::string::FromUtf8Error> {
        String::from_utf8(self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone())
    }
}

/// Share one actual callsite across controlled external subscriber contexts.
fn capture_probe(marker: &str) {
    tracing::error!(operation = "incident_log_capture_fixture", marker, "Scoped capture probe");
}

#[test]
fn unsubscribed_first_registration_preserves_scoped_capture_and_nested_sink_isolation()
-> Result<(), std::string::FromUtf8Error> {
    let outer = Output::default();
    let _outer_capture = outer.observe();
    tracing::dispatcher::with_default(&tracing::Dispatch::none(), || {
        capture_probe("unsubscribed-first");
    });
    capture_probe("outer-before");
    let nested = Output::default();
    {
        let _nested_capture = nested.observe();
        capture_probe("nested-only");
    }
    capture_probe("outer-after");
    let outer_text = outer.text()?;
    let nested_text = nested.text()?;
    assert!(outer_text.contains("outer-before"));
    assert!(outer_text.contains("outer-after"));
    assert!(!outer_text.contains("unsubscribed-first"));
    assert!(!outer_text.contains("nested-only"));
    assert!(nested_text.contains("nested-only"));
    assert!(!nested_text.contains("outer-before"));
    assert!(!nested_text.contains("outer-after"));
    assert!(!nested_text.contains("unsubscribed-first"));
    Ok(())
}
