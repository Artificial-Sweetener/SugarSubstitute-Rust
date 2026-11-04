// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Capture the application formatter's canvas diagnostics on one test thread while other tests
//! run in parallel without a subscriber.

use std::error::Error;
use std::sync::{Arc, Mutex};

/// Collect formatted diagnostic lines written on the test thread.
#[derive(Clone, Default)]
pub(super) struct DiagnosticLines(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for DiagnosticLines {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("Diagnostic lines poisoned"))?
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The installed thread-local formatter and the extra registrar that keeps its callsite
/// interest stable; dropping it ends the capture.
pub(super) struct DiagnosticCapture {
    _subscriber: tracing::subscriber::DefaultGuard,
    _callsite_registry: tracing::Dispatch,
}

impl DiagnosticLines {
    /// Install the application formatter on this thread, writing into these lines.
    ///
    /// The pinned tracing-core single-registrar path computes a newly registered callsite's
    /// shared interest from the registering thread's default. While this capture is the only
    /// live registrar, a parallel test thread without a subscriber that first reaches a canvas
    /// callsite caches it as never, and this capture silently loses that callsite's events. A
    /// second live registrar, never installed and receiving no events, keeps registration on
    /// the shared registrar list.
    pub(super) fn observe(&self) -> DiagnosticCapture {
        let callsite_registry = tracing::Dispatch::new(tracing_subscriber::registry());
        let writer = self.clone();
        let subscriber =
            tracing::subscriber::set_default(crate::diagnostics::fixture_subscriber(move || {
                writer.clone()
            }));
        DiagnosticCapture { _subscriber: subscriber, _callsite_registry: callsite_registry }
    }

    /// The captured text.
    fn text(&self) -> Result<String, Box<dyn Error>> {
        let bytes = self.0.lock().map_err(|_| "Diagnostic lines poisoned")?.clone();
        Ok(String::from_utf8(bytes)?)
    }

    /// Decode the canvas surface lifecycle records of one request, in emission order.
    pub(super) fn requests(
        &self,
        request_id: u64,
    ) -> Result<Vec<crate::canvas_surface_diagnostic::Record>, Box<dyn Error>> {
        Ok(self
            .text()?
            .lines()
            .filter_map(|line| crate::canvas_surface_diagnostic::Record::parse(line.as_bytes()))
            .filter(|record| record.request_id == request_id)
            .collect())
    }
}

/// One callsite that this module's test alone reaches.
fn capture_probe(marker: &str) {
    tracing::info!(operation = "canvas_capture_probe", marker, "Canvas capture probe");
}

#[test]
fn a_capture_keeps_a_callsite_that_an_unsubscribed_thread_reached_first()
-> Result<(), Box<dyn Error>> {
    let lines = DiagnosticLines::default();
    let capture = lines.observe();
    // A thread with no subscriber registers the callsite first, as a parallel test does.
    tracing::dispatcher::with_default(&tracing::Dispatch::none(), || {
        capture_probe("unsubscribed-first");
    });
    capture_probe("captured");
    drop(capture);
    let text = lines.text()?;
    assert!(text.contains("captured"), "{text}");
    assert!(!text.contains("unsubscribed-first"), "{text}");
    Ok(())
}
