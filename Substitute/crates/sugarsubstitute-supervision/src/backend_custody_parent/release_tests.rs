// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe real parent release state after transport closure; wire fixtures grant no peer authority.
use super::*;
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicU64, Ordering},
};
use sugarsubstitute_contracts::{execution::*, lifecycle::*};
use sugarsubstitute_execution::ExecutionOwner;

struct ClockSource(AtomicU64);
impl Clock for ClockSource {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::Acquire))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
/// Count the execution owner's completion wakes so shutdown can be awaited without polling.
#[derive(Default)]
struct Wake {
    revision: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *crate::registry::lock(&self.revision) += 1;
        self.changed.notify_all();
        Ok(())
    }
}

#[test]
fn original_release_receipt_survives_closed_transport_without_new_admission()
-> Result<(), Box<dyn std::error::Error>> {
    let clock = Arc::new(ClockSource(AtomicU64::new(0)));
    let wake = Arc::new(Wake::default());
    let mut execution = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = execution.client();
    let scope =
        client.create_native_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([137; 32]),
        epoch: CoordinatorEpoch::new([137; 16])?,
    };
    let key = BackendCustodyKey::new(owner, BackendCustodySession::new(1)?);
    let resource =
        ResourceKey { owner, id: ResourceId::new(1)?, generation: ResourceGeneration::new(1)? };
    let until = Moment::from_nanos(30_000_000_000);
    let parent = crate::backend_custody_parent::observation_fixture::parent(
        &client,
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::NativeObservation,
            deadline: None,
            replacement: None,
            reservation_bytes:
                sugarsubstitute_execution::windows_lifecycle_pipe::MIN_LIFECYCLE_PIPE_BYTES,
        },
        key,
        until,
    )?;
    parent.access(|state| {
        state.registrations[0] =
            Some(Registration { resource, stop: Some((until, Some(3))), released: None });
        Ok(())
    })?;
    assert!(!parent.advance_finish(&client, until)?);
    assert_eq!(parent.finish_status(), BackendCustodyFinishStatus::Unissued);
    crate::backend_custody_parent::observation_fixture::reply(
        &parent,
        3,
        BackendCustodyCommand::Stop { resource, until_ticks: 30, frequency: 1 },
        BackendCustodyReceipt::Released(resource),
    )?;
    parent.close()?;
    assert!(parent.validate_current_admission().is_err());
    clock.0.store(60_000_000_000, Ordering::Release);
    for _ in 0..2 {
        assert!(parent.advance_resource_release(&client, resource, until)?);
        assert_eq!(parent.release_acknowledgement(resource).ok_or("Receipt lost")?.nonce(), 3);
    }
    assert!(
        parent
            .advance_resource_release(&client, resource, Moment::from_nanos(31_000_000_000))
            .is_err()
    );
    drop(parent);
    execution.request_shutdown(Moment::from_nanos(70_000_000_000));
    loop {
        let observed = *crate::registry::lock(&wake.revision);
        client.drain_completions(NonZeroUsize::MIN)?;
        if execution.poll_shutdown()?.settled {
            break;
        }
        // The execution owner publishes a wake after its last worker returns from shutdown.
        drop(
            wake.changed
                .wait_while(crate::registry::lock(&wake.revision), |current| *current == observed)
                .map_err(|_| "Shutdown wake poisoned")?,
        );
    }
    execution.finish_shutdown()?;
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
