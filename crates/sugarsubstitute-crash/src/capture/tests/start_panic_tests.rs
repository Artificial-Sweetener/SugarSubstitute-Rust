// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve returned registration causes before later cache callbacks can unwind.
use super::{loan_tests::*, *};
use sugarsubstitute_contracts::crash::CaptureOperation;

#[test]
fn registration_failure_survives_later_cache_callback_panic_and_checked_close() -> TestResult {
    let rig = Rig::new()?;
    let port = Arc::new(FakePort::default());
    let first = CrashError::new(
        CaptureOperation::Registration,
        std::io::Error::other("original-registration-failure"),
    );
    *lock(&port.start_failure) = Some(first.clone());
    *lock(&port.build_probe) = Some(Box::new(|| {
        std::panic::resume_unwind(Box::new("original-cache-panic".to_owned()));
    }));
    port.release.open();
    let mut capture = start(&rig, &port)?;
    let shared = capture.shared.as_ref().ok_or("Original shared owner missing")?.clone();
    let JobOutcome::Panicked(panic) = terminal(&rig, &mut capture)? else {
        return Err("Original cache panic missing".into());
    };
    assert_eq!(panic.as_str(), "original-cache-panic");
    let original = lease_address(&shared)?;
    {
        let state = lock(&shared.native);
        assert!(state.errors.iter().flatten().next().is_some_and(|cause| cause.same_cause(&first)));
        assert!(!state.registered && !state.retired && !state.borrowed);
    }
    assert_eq!(port.dropped.load(Ordering::Acquire), 0);
    assert!(rig.scope.snapshot().reserved_bytes >= crate::input::STORAGE_BYTES as u64);
    let cleanup =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?;
    capture.submit_close(
        &rig.client,
        &cleanup,
        close_spec(3, Moment::from_nanos(60_000_000_000))?,
    )?;
    rig.observe_until(&mut capture, |capture| capture.snapshot().state == CaptureState::Closed)?;
    assert_eq!(port.retire_address.load(Ordering::Acquire), original);
    assert!(capture.errors().any(|cause| cause.same_cause(&first)));
    Ok(())
}
