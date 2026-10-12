// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind decoded full identities only to exact actual inventory/read custody.
use super::*;
fn candidate(
    filesystem: &Filesystem,
    object: DiagnosticObject,
    facts: DiagnosticReadFacts,
) -> TestResult {
    let (class, digest) = crate::protected_storage::record_file::diagnostic_object_binding(
        object,
        owner_key(2)?.namespace,
    );
    filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).observations.push(
        crate::diagnostic_storage::inventory::ObservedEntry {
            digest,
            class,
            identity: facts.identity,
            bytes: facts.bytes,
            protected_regular: true,
            payload_group: None,
        },
    );
    Ok(())
}
#[test]
fn diagnostic_seed_and_summary_bind_exact_full_key_without_reopen_and_keep_failed_close()
-> TestResult {
    for seed in [false, true] {
        admitted(move |context, _| {
            let filesystem = Filesystem::new();
            let object = if seed {
                DiagnosticObject::Seed(incident()?)
            } else {
                DiagnosticObject::Summary(incident()?)
            };
            let facts = filesystem.put(object, b"candidate");
            candidate(&filesystem, object, facts)?;
            let (mut storage, operation) = setup(&context, filesystem.clone())?;
            let inventory = storage.inventory(&operation, &context)?;
            let entry = inventory.entries().next().ok_or("missing candidate")?;
            let mut lease = storage.read_inventory(entry, 64, &operation, &context)?;
            assert_eq!(storage.snapshot().live_handles(), 1);
            lease.bind_object(object, &context)?;
            lease.bind_object(object, &context)?;
            assert_eq!(storage.snapshot().live_handles(), 1);
            let changed = if seed {
                DiagnosticObject::Summary(incident()?)
            } else {
                DiagnosticObject::Seed(incident()?)
            };
            assert_eq!(
                lease.bind_object(changed, &context).err().ok_or("changed repeat bound")?.code(),
                Code::ChangedIdentity
            );
            filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.close =
                true;
            assert!(lease.retire(&context).is_err());
            lease.bind_object(object, &context)?;
            assert_eq!(storage.snapshot().reads(), 1);
            assert_eq!(storage.snapshot().live_handles(), 1);
            filesystem.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).faults.close =
                false;
            lease.retire(&context)?;
            assert!(lease.bind_object(object, &context).is_err());
            storage.retire(&operation, &context)?;
            Ok(())
        })?;
    }
    Ok(())
}
#[test]
fn diagnostic_inventory_binding_rejects_wrong_class_name_and_full_key() -> TestResult {
    admitted(|context, _| {
        let filesystem = Filesystem::new();
        let object = DiagnosticObject::Summary(incident()?);
        let facts = filesystem.put(object, b"candidate");
        candidate(&filesystem, object, facts)?;
        let (mut storage, operation) = setup(&context, filesystem.clone())?;
        let inventory = storage.inventory(&operation, &context)?;
        let mut lease = storage.read_inventory(
            inventory.entries().next().ok_or("missing candidate")?,
            64,
            &operation,
            &context,
        )?;
        for wrong in [
            DiagnosticObject::Seed(incident()?),
            DiagnosticObject::Summary(IncidentKey::new(IncidentSubject::Supervisor(owner_key(3)?))),
        ] {
            assert_eq!(
                lease
                    .bind_object(wrong, &context)
                    .err()
                    .ok_or("wrong canonical binding accepted")?
                    .code(),
                Code::InvalidIdentity
            );
        }
        assert_eq!(storage.snapshot().live_handles(), 1);
        storage.retire(&operation, &context)?;
        Ok(())
    })
}

#[test]
fn diagnostic_binding_deadline_retains_original_operation_for_actual_cleanup() -> TestResult {
    let wake = Arc::new(Wake::default());
    let clock = Arc::new(TestClock(AtomicU64::new(17)));
    let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let retained = Arc::new(Mutex::new(None));
    let output = retained.clone();
    let job_clock = clock.clone();
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(100)),
            reservation_bytes: DIAGNOSTIC_STORAGE_BYTES,
            replacement: None,
        },
        move |context| -> TestResult {
            let clock = job_clock;
            let filesystem = Filesystem::new();
            let object = DiagnosticObject::Summary(incident()?);
            filesystem.put(object, b"summary");
            let (mut storage, operation) = setup(&context, filesystem.clone())?;
            let mut lease = storage.read(object, 64, &operation, &context)?;
            clock.0.store(100, Ordering::SeqCst);
            assert_eq!(
                lease.bind_object(object, &context).err().ok_or("expired binding accepted")?.code(),
                Code::Deadline
            );
            drop(lease);
            assert_eq!(storage.snapshot().live_handles(), 1);
            assert_eq!(storage.snapshot().reads(), 1);
            assert_eq!(
                storage
                    .shared
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .operation
                    .as_ref()
                    .ok_or("operation missing")?
                    .last_observed,
                Moment::from_nanos(100)
            );
            assert_eq!(
                storage
                    .retire(&operation, &context)
                    .err()
                    .ok_or("expired cleanup admitted")?
                    .code(),
                Code::Deadline
            );
            *output.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some((storage, operation));
            Ok(())
        },
    )?;
    assert!(matches!(observe_job(&client, &wake, &mut job)?, JobOutcome::TimedOut));
    drop(job);
    assert_eq!(scope.snapshot().reserved_bytes, DIAGNOSTIC_STORAGE_BYTES);
    let cleanup = client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let input = retained.clone();
    let mut cleanup_job = client.submit_blocking(
        &cleanup,
        JobSpec {
            id: JobId::new(2)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(200)),
            reservation_bytes: 0,
            replacement: None,
        },
        move |context| -> TestResult {
            let (mut storage, operation) = input
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
                .ok_or("retained owner missing")?;
            storage.retire(&operation, &context)?;
            assert_eq!(storage.snapshot().live_handles(), 0);
            assert_eq!(storage.snapshot().cleanup_deadline(), Some(Moment::from_nanos(200)));
            assert_eq!(storage.snapshot().deadline(), Some(Moment::from_nanos(100)));
            assert_eq!(context.now(), Moment::from_nanos(100));
            Ok(())
        },
    )?;
    assert!(matches!(observe_job(&client, &wake, &mut cleanup_job)?, JobOutcome::Success(())));
    drop(cleanup_job);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    drop(owner);
    Ok(())
}

#[test]
fn diagnostic_binding_rejects_foreign_scope_and_keeps_actual_owner_for_cleanup() -> TestResult {
    let wake = Arc::new(Wake::default());
    let clock = Arc::new(TestClock(AtomicU64::new(17)));
    let owner = ExecutionOwner::new(clock, wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let retained = Arc::new(Mutex::new(None));
    let output = retained.clone();
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(100)),
            reservation_bytes: DIAGNOSTIC_STORAGE_BYTES,
            replacement: None,
        },
        move |context| -> TestResult {
            let filesystem = Filesystem::new();
            let object = DiagnosticObject::Summary(incident()?);
            let facts = filesystem.put(object, b"summary");
            candidate(&filesystem, object, facts)?;
            let (mut storage, operation) = setup(&context, filesystem)?;
            let inventory = storage.inventory(&operation, &context)?;
            let lease = storage.read_inventory(
                inventory.entries().next().ok_or("candidate missing")?,
                64,
                &operation,
                &context,
            )?;
            *output.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some((storage, operation, lease));
            Ok(())
        },
    )?;
    assert!(matches!(observe_job(&client, &wake, &mut job)?, JobOutcome::Success(())));
    drop(job);
    assert_eq!(scope.snapshot().reserved_bytes, DIAGNOSTIC_STORAGE_BYTES);
    let foreign = client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let mut job = client.submit_blocking(
        &foreign,
        JobSpec {
            id: JobId::new(2)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(100)),
            reservation_bytes: 0,
            replacement: None,
        },
        move |context| -> TestResult {
            let (mut storage, operation, mut lease) = retained
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
                .ok_or("retained source missing")?;
            assert_eq!(
                lease
                    .bind_object(DiagnosticObject::Summary(incident()?), &context)
                    .err()
                    .ok_or("foreign scope bound")?
                    .code(),
                Code::InvalidIdentity
            );
            assert_eq!(storage.snapshot().live_handles(), 1);
            assert_eq!(storage.snapshot().reads(), 1);
            storage.retire(&operation, &context)?;
            assert_eq!(storage.snapshot().live_handles(), 0);
            assert_eq!(storage.snapshot().cleanup_deadline(), Some(Moment::from_nanos(100)));
            assert_eq!(operation.original_deadline(), Moment::from_nanos(100));
            Ok(())
        },
    )?;
    assert!(matches!(observe_job(&client, &wake, &mut job)?, JobOutcome::Success(())));
    drop(job);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    drop(owner);
    Ok(())
}
