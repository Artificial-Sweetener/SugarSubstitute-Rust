// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify actual production client setup with restricted read-only bootstrap ownership.
use super::*;
use crate::runtime_test_support::{Rig, TestResult, spec};
use std::os::windows::io::{AsRawHandle, IntoRawHandle};
use sugarsubstitute_contracts::{execution::*, lifecycle::*};

#[test]
fn client_access_changes_message_mode_without_granting_another_pipe_instance() {
    use windows::Win32::{Foundation::GENERIC_WRITE, Storage::FileSystem::*};
    assert_ne!(native::CLIENT_ACCESS & FILE_WRITE_ATTRIBUTES.0, 0);
    assert_eq!(native::CLIENT_ACCESS & FILE_APPEND_DATA.0, 0);
    assert_eq!(native::CLIENT_ACCESS & GENERIC_WRITE.0, 0);
}

#[test]
fn ffi_actual_client_prepares_message_mode_and_retires_both_native_sessions() -> TestResult {
    let rig = Rig::new()?;
    let scope =
        rig.client.create_native_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let mut namespace = [73; 32];
    let (ticks, _) = native::counter::sample()?;
    namespace[..8].copy_from_slice(&ticks.to_le_bytes());
    // SAFETY: current-process identity needs no borrowed handle or mutable native output.
    namespace[8..12].copy_from_slice(
        &unsafe { windows::Win32::System::Threading::GetCurrentProcessId() }.to_le_bytes(),
    );
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new(namespace),
            epoch: CoordinatorEpoch::new([74; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    };
    let specification = |id| -> TestResult<JobSpec> {
        let mut value = spec(id, JobClass::NativeObservation, MIN_LIFECYCLE_PIPE_BYTES)?;
        value.operation = Operation::NativeLifecycle;
        Ok(value)
    };
    let mut server = rig.client.reserve_lifecycle_pipe_server(
        &scope,
        specification(1)?,
        run,
        Moment::from_nanos(10_000_000_000),
    )?;
    let bootstrap = loop {
        let before = rig.wake.count();
        rig.client.drain_completions(std::num::NonZeroUsize::MIN)?;
        if let Some(bootstrap) = server.control.take_bootstrap()? {
            break bootstrap;
        }
        if let Some(completion) = server.completion.try_take_completion() {
            return Err(
                format!("Actual server preparation failed: {:?}", completion.outcome).into()
            );
        }
        rig.wake.wait_after(before)?;
    };
    let original = bootstrap.deadline().ok_or("Actual bootstrap deadline missing")?;
    let permit = rig.client.reserve_lifecycle_pipe_client(&scope, specification(2)?, original)?;
    let duplicate = bootstrap.borrow_inheritance().section.try_clone_to_owned()?;
    let token = InheritedLifecycleBootstrap::new(duplicate.as_raw_handle() as u64)?;
    let _transferred = duplicate.into_raw_handle();
    let mut client = permit.bind(token).ok_or("Fresh admitted client permit lost")?;
    let preparation = loop {
        let before = rig.wake.count();
        rig.client.drain_completions(std::num::NonZeroUsize::MIN)?;
        if client.control.prepared_run() == Some(run) {
            break Ok(());
        }
        if let Some(completion) = client.completion.try_take_completion() {
            break match completion.outcome {
                JobOutcome::Failure(error) => Err(Box::new(error) as Box<dyn std::error::Error>),
                other => Err(format!("Actual client preparation ended: {other:?}").into()),
            };
        }
        rig.wake.wait_after(before)?;
    };
    client.control.close();
    server.control.close();
    drop(bootstrap);
    if preparation.is_ok() {
        assert!(matches!(rig.completion(&mut client.completion)?.outcome, JobOutcome::Cancelled));
    }
    assert!(matches!(rig.completion(&mut server.completion)?.outcome, JobOutcome::Cancelled));
    preparation
}
