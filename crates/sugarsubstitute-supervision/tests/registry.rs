// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Public pure input/state contracts without native child creation.
#![cfg(windows)]
use std::error::Error;
use sugarsubstitute_contracts::lifecycle::{
    CoordinatorEpoch, CoordinatorKey, NamespaceId, ProcessId, ProcessKey,
};
use sugarsubstitute_supervision::{
    LaunchEnvironment, RegistryAdmissionError, WindowsLaunchCommand, WindowsLaunchInput,
    WindowsProcessRegistry,
};
type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn public_launch_input_charges_native_buffers_and_independent_family_storage() -> TestResult {
    let executable: Vec<_> = "E:\\fixture\\sugarsubstitute.exe".encode_utf16().collect();
    let cwd: Vec<_> = "E:\\fixture".encode_utf16().collect();
    let command = WindowsLaunchCommand::from_utf16(
        &executable,
        &cwd,
        &[Vec::new(), "雪".encode_utf16().collect()],
    )?;
    let buffers = command.into_native_parts();
    let expected = ((buffers.application_name.capacity()
        + buffers.working_directory.capacity()
        + buffers.command_line.capacity())
        * 2) as u64;
    let environment = LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?;
    let environment_bytes = environment.allocated_bytes()?;
    let command = WindowsLaunchCommand::from_utf16(
        &executable,
        &cwd,
        &[Vec::new(), "雪".encode_utf16().collect()],
    )?;
    let input = WindowsLaunchInput::new(command, environment)?;
    assert!(input.required_reservation_bytes() > expected + environment_bytes);
    assert!(!format!("{input:?}").contains("fixture"));
    Ok(())
}

#[test]
fn public_registry_matches_exact_coordinator_without_pid_authority() -> TestResult {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([1; 16])?,
    };
    let mut fixture = RegistryFixture::new(owner)?;
    let registry = &mut fixture.registry;
    assert!(registry.is_empty());
    let key = ProcessKey { owner, id: ProcessId::new(1)? };
    assert!(matches!(registry.snapshot(key), Err(RegistryAdmissionError::Stale)));
    let mut foreign = key;
    foreign.owner.epoch = CoordinatorEpoch::new([2; 16])?;
    assert!(matches!(registry.snapshot(foreign), Err(RegistryAdmissionError::ForeignOwner)));
    registry.close_launch_admission();
    assert_eq!(registry.next_cleanup(), None);
    assert!(registry.is_empty());
    Ok(())
}

/// Retain the original execution owner after actual admitted table construction.
struct RegistryFixture {
    registry: WindowsProcessRegistry,
    _scope: sugarsubstitute_contracts::execution::Scope,
    _owner: sugarsubstitute_execution::ExecutionOwner,
}
#[derive(Default)]
struct Wake {
    revision: std::sync::Mutex<u64>,
    changed: std::sync::Condvar,
}
impl sugarsubstitute_contracts::execution::CompletionWake for Wake {
    fn wake(&self) -> Result<(), sugarsubstitute_contracts::execution::ExecutionError> {
        *self.revision.lock().map_err(|_| {
            sugarsubstitute_contracts::execution::ExecutionError::new(
                sugarsubstitute_contracts::execution::Operation::NativeLifecycle,
                Box::new(std::io::Error::other("Public registry wake poisoned")),
            )
        })? += 1;
        self.changed.notify_all();
        Ok(())
    }
}
struct Clock;
impl sugarsubstitute_contracts::execution::Clock for Clock {
    fn now(&self) -> sugarsubstitute_contracts::execution::Moment {
        sugarsubstitute_contracts::execution::Moment::from_nanos(0)
    }
    fn sleep_until(
        &self,
        _: sugarsubstitute_contracts::execution::Moment,
    ) -> sugarsubstitute_contracts::execution::WaitFuture {
        Box::pin(std::future::pending())
    }
}
impl RegistryFixture {
    /// Consume the genuine typed success; no fabricated lease or native effect is supplied.
    fn new(binding: CoordinatorKey) -> Result<Self, Box<dyn Error>> {
        use sugarsubstitute_contracts::execution::*;
        let wake = std::sync::Arc::new(Wake::default());
        let owner = sugarsubstitute_execution::ExecutionOwner::new(
            std::sync::Arc::new(Clock),
            wake.clone(),
        )?;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let spec = JobSpec {
            id: JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: None,
            reservation_bytes: WindowsProcessRegistry::required_reservation_bytes(),
            replacement: None,
        };
        let mut job = WindowsProcessRegistry::submit_new(&client, &scope, spec, binding)?;
        loop {
            let revision = *wake.revision.lock().map_err(|_| "Public constructor wake poisoned")?;
            let batch = client.drain_completions(
                std::num::NonZeroUsize::new(32).ok_or("Invalid public drain")?,
            )?;
            assert!(batch.notifications.iter().all(|notice| notice.job_id == spec.id));
            if let Some(completion) = job.try_take_completion() {
                assert_eq!(completion.scope_id, scope.id());
                assert_eq!(completion.spec, spec);
                let registry = match completion.outcome {
                    JobOutcome::Success(value) => value,
                    JobOutcome::Failure(cause) => return Err(Box::new(cause)),
                    JobOutcome::Panicked(cause) => {
                        return Err(std::io::Error::other(cause.as_str()).into());
                    }
                    _ => return Err("Actual public constructor stopped".into()),
                };
                return Ok(Self { registry, _scope: scope, _owner: owner });
            }
            let guard = wake.revision.lock().map_err(|_| "Public constructor wake poisoned")?;
            let (_guard, timed) = wake
                .changed
                .wait_timeout_while(guard, std::time::Duration::from_secs(10), |value| {
                    *value == revision
                })
                .map_err(|_| "Public constructor wake poisoned")?;
            if timed.timed_out() {
                return Err("Actual public constructor completion missing".into());
            }
        }
    }
}
