// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the actual returned primary and original charge through lazy post observation.
use super::*;
use std::sync::atomic::AtomicBool;
struct Primary {
    original: MemoryLease,
    dropped: Arc<AtomicBool>,
}
impl std::fmt::Debug for Primary {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("Primary")
    }
}
impl std::fmt::Display for Primary {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("external primary")
    }
}
impl std::error::Error for Primary {}
impl Drop for Primary {
    fn drop(&mut self) {
        assert!(
            self.original.claim(DIAGNOSTIC_STORAGE_BYTES).is_err(),
            "primary data outlived original custody"
        );
        self.dropped.store(true, Ordering::SeqCst);
    }
}
#[test]
fn capture_error_original_cause_drops_before_its_last_whole_guard() -> TestResult {
    admitted(|context, _| {
        let (mut storage, operation) = setup(&context, Filesystem::new())?;
        let dropped = Arc::new(AtomicBool::new(false));
        let primary = Primary { original: context.memory().clone(), dropped: dropped.clone() };
        let slot = capture_intent::types::CaptureErrorSlot::new(&storage.shared, operation.key())?;
        let cause = capture_intent::retirement::observe_source(
            &storage.shared,
            Err(primary),
            || Err(error(operation.key(), Code::Deadline)),
            slot,
        )
        .err()
        .ok_or("source lost")?;
        assert!(cause.source_error().is_some());
        assert_eq!(cause.post_storage_error().ok_or("post lost")?.code(), Code::Deadline);
        storage.retire(&operation, &context)?;
        drop(operation);
        drop(storage);
        assert!(context.memory().claim(DIAGNOSTIC_STORAGE_BYTES).is_err());
        drop(cause);
        assert!(dropped.load(Ordering::SeqCst));
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}
#[expect(
    clippy::panic,
    reason = "Exercise the actual adopted source result across an external post-provider unwind"
)]
#[test]
fn capture_error_post_provider_panic_preserves_primary_and_original_charge() -> TestResult {
    admitted(|context, _| {
        let (mut storage, operation) = setup(&context, Filesystem::new())?;
        let dropped = Arc::new(AtomicBool::new(false));
        let primary = Primary { original: context.memory().clone(), dropped: dropped.clone() };
        let slot = capture_intent::types::CaptureErrorSlot::new(&storage.shared, operation.key())?;
        let cause = capture_intent::retirement::observe_source(
            &storage.shared,
            Err(primary),
            || panic!("external post provider"),
            slot,
        )
        .err()
        .ok_or("panic primary lost")?;
        assert!(cause.source_error().is_some());
        assert!(cause.post_panicked());
        assert!(storage.snapshot().admission_closed());
        storage.retire(&operation, &context)?;
        drop(operation);
        drop(storage);
        assert!(context.memory().claim(DIAGNOSTIC_STORAGE_BYTES).is_err());
        drop(cause);
        assert!(dropped.load(Ordering::SeqCst));
        assert_eq!(
            context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?.bytes(),
            DIAGNOSTIC_STORAGE_BYTES
        );
        Ok(())
    })
}
