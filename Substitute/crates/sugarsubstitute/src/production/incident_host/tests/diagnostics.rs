// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe actual tracing output through an isolated external writer.
use std::{
    io::{self, Write},
    sync::{Arc, LazyLock, Mutex},
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
/// Callsite interest registrars that live for the whole test process and are never installed.
///
/// tracing-core computes a callsite's interest from the registering thread's own default
/// whenever at most one dispatcher is registered, and caches the result. A capture that is the
/// only live dispatcher therefore loses every callsite another thread registers, or rebuilds,
/// without a subscriber: the callsite caches `never` and the capture sees nothing. Two permanent
/// registrars keep the dispatcher count above one, so interest is always the union over every
/// live dispatcher, whichever thread registers. They receive no events or causes.
static CALLSITE_REGISTRARS: LazyLock<[tracing::Dispatch; 2]> = LazyLock::new(|| {
    [
        tracing::Dispatch::new(tracing_subscriber::registry()),
        tracing::Dispatch::new(tracing_subscriber::registry()),
    ]
});

/// Retain the installed formatter until it has left this thread.
pub(in crate::production) struct ScopedCapture {
    _subscriber: tracing::subscriber::DefaultGuard,
}
impl Output {
    /// Install a thread-local formatter with independent output and stable callsite interest.
    pub(in crate::production) fn observe(&self) -> ScopedCapture {
        // Registered before this formatter, so no registration ever samples a thread default.
        LazyLock::force(&CALLSITE_REGISTRARS);
        let output = self.clone();
        let subscriber = tracing::subscriber::set_default(
            tracing_subscriber::fmt()
                .without_time()
                .with_ansi(false)
                .with_writer(move || output.clone())
                .finish(),
        );
        ScopedCapture { _subscriber: subscriber }
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

/// A capture that is the only subscriber anywhere keeps its callsites when another context
/// without a subscriber rebuilds callsite interest, as a first registration on an unsubscribed
/// thread does under parallel tests.
#[test]
fn a_lone_capture_keeps_interest_rebuilt_from_an_unsubscribed_context()
-> Result<(), std::string::FromUtf8Error> {
    let output = Output::default();
    let _capture = output.observe();
    capture_probe("before-rebuild");
    tracing::dispatcher::with_default(&tracing::Dispatch::none(), || {
        tracing::callsite::rebuild_interest_cache();
        capture_probe("unsubscribed-rebuild");
    });
    capture_probe("after-rebuild");
    let text = output.text()?;
    assert!(text.contains("before-rebuild"));
    assert!(text.contains("after-rebuild"), "{text}");
    assert!(!text.contains("unsubscribed-rebuild"));
    Ok(())
}
