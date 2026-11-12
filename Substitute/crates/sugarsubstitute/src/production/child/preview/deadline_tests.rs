// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe original deadline rejection and sticky cause on the actual preview owner.
use super::*;
use std::{io::Write, sync::Mutex};

#[derive(Clone)]
struct Sink(Arc<Mutex<Vec<u8>>>);
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Sink {
    type Writer = Self;
    fn make_writer(&'a self) -> Self {
        self.clone()
    }
}
impl Write for Sink {
    fn write(&mut self, value: &[u8]) -> std::io::Result<usize> {
        let mut bytes =
            self.0.lock().map_err(|_| std::io::Error::other("Trace fixture poisoned"))?;
        if bytes.len().saturating_add(value.len()) > 8192 {
            return Err(std::io::Error::other("Trace fixture capacity"));
        }
        bytes.extend_from_slice(value);
        Ok(value.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn recorded(emit: impl FnOnce() -> TestResult) -> Result<String, Box<dyn Error>> {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_writer(Sink(bytes.clone()))
        .without_time()
        .with_ansi(false)
        .finish();
    tracing::subscriber::with_default(subscriber, emit)?;
    let result = bytes.lock().map_err(|_| "Trace fixture poisoned")?.clone();
    Ok(String::from_utf8(result)?)
}

#[test]
fn unissued_preview_deadline_is_typed_and_reported_exactly_once() -> TestResult {
    let mut owner = preview()?;
    let trace = recorded(|| {
        owner.poll(Moment::from_nanos(99))?;
        assert!(owner.failure.is_none());
        owner.poll(Moment::from_nanos(100))?;
        owner.poll(Moment::from_nanos(101))?;
        Ok(())
    })?;
    assert!(matches!(owner.failure, Some(PreviewFailure::Host(PreviewError::Deadline))));
    assert_eq!(trace.matches("operation=\"main_preview_qualification_failed\"").count(), 1);
    assert!(trace.contains("cause_kind=\"Deadline\""));
    assert!(!owner.requested && !owner.received && !owner.closing);
    assert!(owner.job.is_none() && owner.terminal.is_none());
    assert_eq!(owner.until, Moment::from_nanos(100));
    Ok(())
}

#[test]
fn requested_preview_deadline_preserves_late_actual_native_failure_and_original_cutoff()
-> TestResult {
    let runtime = ExecutionOwner::new(Arc::new(Fixed), Arc::new(Wake))?;
    let client = runtime.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut owner = preview()?;
    let identity = main()?;
    owner
        .after_main_revealed(identity, &session(true)?, &scope, &scope, Moment::from_nanos(17))?
        .ok_or("Actual current preview request absent")?;
    let trace = recorded(|| {
        owner.poll(Moment::from_nanos(100))?;
        assert!(!owner.received && !owner.closing);
        let mut native = Some(failed(identity)?);
        assert!(owner.accept(&mut native, &client, &scope, &JobSequence::new())?);
        assert!(native.is_none());
        owner.poll(Moment::from_nanos(101))?;
        Ok(())
    })?;
    assert!(owner.received);
    assert!(matches!(owner.failure, Some(PreviewFailure::Host(PreviewError::Deadline))));
    assert_eq!(trace.matches("operation=\"main_preview_qualification_failed\"").count(), 1);
    assert_eq!(owner.until, Moment::from_nanos(100));
    Ok(())
}

#[test]
fn prior_native_cause_and_explicit_close_do_not_become_deadline_failures() -> TestResult {
    let runtime = ExecutionOwner::new(Arc::new(Fixed), Arc::new(Wake))?;
    let client = runtime.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut owner = preview()?;
    let identity = main()?;
    owner
        .after_main_revealed(identity, &session(true)?, &scope, &scope, Moment::from_nanos(17))?
        .ok_or("Actual preview request absent")?;
    let trace = recorded(|| {
        let mut native = Some(failed(identity)?);
        assert!(owner.accept(&mut native, &client, &scope, &JobSequence::new())?);
        owner.poll(Moment::from_nanos(100))?;
        Ok(())
    })?;
    assert!(matches!(owner.failure, Some(PreviewFailure::Native(_))));
    assert_eq!(trace.matches("operation=\"main_preview_qualification_failed\"").count(), 1);
    assert!(trace.contains("cause_kind=\"Native\""));
    let mut closed = preview()?;
    closed.close();
    closed.poll(Moment::from_nanos(100))?;
    assert!(closed.failure.is_none());
    Ok(())
}
