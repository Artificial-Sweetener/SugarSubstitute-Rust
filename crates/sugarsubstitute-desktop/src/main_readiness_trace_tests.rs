// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check bounded accepted-transition receipts from the real current-document owner.
use super::*;
use std::sync::{Mutex, atomic::AtomicBool};
use tracing::{
    Event, Metadata, Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
};
type Fields = Vec<(String, String)>;
#[derive(Clone, Default)]
struct Sink {
    events: Arc<Mutex<Vec<Fields>>>,
    failed: Arc<AtomicBool>,
}
struct Visitor(Fields);
impl Visit for Visitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0.push((field.name().to_owned(), format!("{value:?}")));
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.push((field.name().to_owned(), value.to_owned()));
    }
}
impl Subscriber for Sink {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.target().ends_with("main_readiness")
    }
    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }
    fn record(&self, _: &Id, _: &Record<'_>) {}
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}
    fn event(&self, event: &Event<'_>) {
        let mut visitor = Visitor(Vec::new());
        event.record(&mut visitor);
        match self.events.lock() {
            Ok(mut events) if events.len() < 4 && visitor.0.len() <= 8 => events.push(visitor.0),
            _ => self.failed.store(true, Ordering::SeqCst),
        }
    }
}
fn captured(emit: impl FnOnce() -> Result) -> Result<Vec<Fields>> {
    let sink = Sink::default();
    tracing::subscriber::with_default(sink.clone(), emit)?;
    assert!(!sink.failed.load(Ordering::SeqCst));
    let events = sink.events.lock().map_err(|_| "Trace fixture poisoned")?.clone();
    Ok(events)
}

#[test]
fn accepted_navigation_and_barrier_receipts_are_once_and_current() -> Result {
    let identity = identity()?;
    let events = captured(|| {
        let mut owner = owner()?;
        let target = loaded(&mut owner)?;
        owner.navigation_completed(true)?;
        owner.dom_result(target, Some(positive(target)?))?;
        owner.barrier_complete(target)?;
        assert_eq!(owner.barrier_complete(target), Err(ReadinessFault::StaleProbe));
        Ok(())
    })?;
    assert_eq!(events.len(), 2);
    assert!(events[0].contains(&("operation".into(), "main_navigation_completed".into())));
    assert!(events[1].contains(&("operation".into(), "main_frontend_receipt".into())));
    assert!(events[1].contains(&("ready".into(), "true".into())));
    for fields in events {
        assert!(fields.contains(&("identity".into(), format!("{identity:?}"))));
    }
    Ok(())
}

#[test]
fn rejected_navigation_or_invalidated_barrier_emits_no_success_receipt() -> Result {
    let events = captured(|| {
        let mut invalid = owner()?;
        assert_eq!(invalid.navigation_completed(true), Err(ReadinessFault::NativeNavigationFailed));
        let mut current = owner()?;
        let target = loaded(&mut current)?;
        current.dom_result(target, Some(positive(target)?))?;
        current.invalidate(ReadinessFault::SetupFailed);
        assert_eq!(current.barrier_complete(target), Err(ReadinessFault::SetupFailed));
        Ok(())
    })?;
    assert_eq!(events.len(), 1);
    assert!(events[0].contains(&("operation".into(), "main_navigation_completed".into())));
    Ok(())
}
