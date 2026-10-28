// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Production Reporter metadata crosses the real central watch/creation admission boundary.
use super::*;
use crate::production::scope_sequence::ScopeSequence;
use std::sync::atomic::{AtomicUsize, Ordering};
use sugarsubstitute_execution::{OwnedInputError, windows::NativeCorrelation};

#[test]
fn production_reporter_jobs_admit_watch_before_creation_and_keep_original_credit_and_cutoff()
-> Result<()> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let jobs = crate::job_sequence::JobSequence::new();
    let scopes = ScopeSequence::new();
    let finite = client.create_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, None)?;
    let until = Moment::from_nanos(60_000_000_017);
    let bytes = 512 * 1024;
    let specs = ReporterLaunch::allocate(
        ScopeSpec { id: scopes.allocate()?, deadline: None },
        &jobs,
        until,
        bytes,
    )?;
    let native = client.create_native_scope(specs.native, None)?;
    let permit = client.reserve_process_watch(&native, specs.watch, NativeCorrelation::new(1)?)?;
    let mut creation = client.submit_blocking(&finite, specs.creation, move |context| {
        assert_eq!(context.spec().deadline, Some(until));
        context.memory().claim(bytes)
    })?;
    // No process handle is bound. Genuine unbound cancellation still delivers an observed terminal.
    let mut watch = permit.cancel_unbound();
    let mut created = None;
    let mut watched = None;
    while created.is_none() || watched.is_none() {
        let _actual = batch(&client, &wake)?;
        if created.is_none() {
            created = creation.try_take_completion();
        }
        if watched.is_none() {
            watched = watch.try_take_completion();
        }
    }
    let created = created.ok_or("Creation terminal absent")?;
    let watched = watched.ok_or("Watch terminal absent")?;
    assert_eq!(created.spec, specs.creation);
    assert_eq!(watched.spec, specs.watch);
    assert!(matches!(created.outcome, JobOutcome::Success(_)));
    assert!(matches!(watched.outcome, JobOutcome::Cancelled));
    assert_eq!(watched.spec.deadline, None);
    assert_eq!(watched.spec.reservation_bytes, 4096);
    drop(creation);
    drop(watch);
    drop(owner);
    assert_eq!(finite.snapshot().reserved_bytes, bytes);
    assert_eq!(native.snapshot().reserved_bytes, 4096);
    drop(created);
    drop(watched);
    assert_eq!(finite.snapshot().reserved_bytes, 0);
    assert_eq!(native.snapshot().reserved_bytes, 0);

    // Reproduce the former Host order against the actual execution consumer, retaining input.
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let finite = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let native = client.create_native_scope(specs.native, None)?;
    let old_watch = JobSpec { id: specs.creation.id, ..specs.watch };
    let old_creation = JobSpec { id: specs.watch.id, ..specs.creation };
    let permit = client.reserve_process_watch(&native, old_watch, NativeCorrelation::new(1)?)?;
    let calls = Arc::new(AtomicUsize::new(0));
    let worker = calls.clone();
    let mut original = Some(17_u64);
    let rejected =
        client.submit_blocking_owned_box(&finite, old_creation, &mut original, move |input, _| {
            worker.fetch_add(1, Ordering::AcqRel);
            Ok::<_, MemoryClaimError>(input)
        });
    assert!(
        matches!(rejected, Err(OwnedInputError::Admission(AdmissionError::JobIdentityOrder { latest })) if latest == old_watch.id)
    );
    assert_eq!(original, Some(17));
    assert_eq!(calls.load(Ordering::Acquire), 0);
    assert_eq!(finite.snapshot().reserved_bytes, 0);
    let mut watch = permit.cancel_unbound();
    let completed = loop {
        let _actual = batch(&client, &wake)?;
        if let Some(done) = watch.try_take_completion() {
            break done;
        }
    };
    assert!(matches!(completed.outcome, JobOutcome::Cancelled));
    drop(watch);
    drop(owner);
    drop(completed);
    assert_eq!(native.snapshot().reserved_bytes, 0);
    Ok(())
}
