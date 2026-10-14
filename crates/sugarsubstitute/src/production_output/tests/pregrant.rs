// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Feed typed raw boundary bytes into the real source/parser while readiness stays absent.
use super::*;
use sugarsubstitute_application::supervision::{ResourceCoordinator, ResourcePhase};
use sugarsubstitute_contracts::configuration::{LaunchConfiguration, ProfileId};
#[test]
fn actual_pending_reservation_ingests_stdout_stderr_into_one_charged_source_before_grant()
-> TestResult {
    let mut rig = Rig::new()?;
    let inert = binding()?;
    let mut coordinator = ResourceCoordinator::new(inert.run.owner);
    coordinator.register_run(ClientRegistration {
        run: inert.run,
        connection: inert.connection,
        launch: sugarsubstitute_contracts::startup::LaunchId::new(1)?,
        profile: ProfileId::new("Progress_1".into())?,
    })?;
    let root = std::env::current_dir()?;
    let transition = coordinator.request(
        ClientBinding { run: inert.run, connection: inert.connection },
        ResourceRequest {
            run: inert.run,
            connection: inert.connection,
            id: RequestId::new(1)?,
            command: ResourceCommand::Acquire(BackendTarget::Managed(LaunchConfiguration::new(
                root.clone(),
                root.join("inert-python.exe"),
                vec![],
                8188,
            )?)),
        },
    )?;
    let RequestOutcome::Pending(lease) =
        transition.replies.first().ok_or("Actual Pending absent")?.outcome
    else {
        return Err("No reservation".into());
    };
    let binding = OutputBinding {
        resource: lease.resource,
        run: lease.run,
        connection: lease.connection,
        lease: LeaseReference {
            resource: lease.resource.id,
            generation: lease.resource.generation,
            lease: lease.id,
        },
    };
    let mut constructor = OutputOwner::submit_new(
        &rig.client,
        &rig.scope,
        JobId::new(1)?,
        binding,
        Moment::from_nanos(100),
    )?;
    let owner = rig.result(&mut constructor)?;
    drop(constructor);
    assert_eq!(coordinator.resource_phase(lease.resource), Some(ResourcePhase::Preparing));
    let source = owner.source.clone();
    let mut parser = rig.client.submit_blocking(
        &rig.scope,
        spec(JobId::new(2)?, Moment::from_nanos(100), TRANSFER_BYTES),
        move |context| {
            let _transfer = context.memory().claim(TRANSFER_BYTES).map_err(OutputError::Claim)?;
            source.check(&context)?;
            let mut state = source.state.lock().map_err(|_| OutputError::Poisoned)?;
            let first = worker::apply_delivery(
                &mut state.transcript,
                binding,
                OutputDelivery {
                    lease: binding.lease,
                    delivery_id: NonZeroU64::new(1).ok_or(OutputError::Identity)?,
                    stream: OutputStream::Stdout,
                    sequence: StreamSequence::new(1)?,
                    gap: None,
                    end: false,
                    bytes: b"loading model\n",
                },
                &source.custody,
            )?;
            drop(first);
            worker::apply_delivery(
                &mut state.transcript,
                binding,
                OutputDelivery {
                    lease: binding.lease,
                    delivery_id: NonZeroU64::new(2).ok_or(OutputError::Identity)?,
                    stream: OutputStream::Stderr,
                    sequence: StreamSequence::new(1)?,
                    gap: None,
                    end: false,
                    bytes: b"\x1b[33mcompiling kernels\x1b[0m\n",
                },
                &source.custody,
            )
            .map(|parsed| parsed.snapshot)
        },
    )?;
    let snapshot = rig.result(&mut parser)?.ok_or("Snapshot absent")?;
    let records: Vec<_> =
        snapshot.transcript().records().map(|record| record.text().to_owned()).collect();
    assert_eq!(records, ["loading model", "compiling kernels"]);
    assert_eq!(coordinator.resource_phase(lease.resource), Some(ResourcePhase::Preparing));
    assert!(owner.acknowledged.is_none());
    assert_eq!(snapshot.source_bytes(), SOURCE_BYTES);
    drop((parser, owner));
    rig.finish()?;
    assert_eq!(rig.scope.snapshot().reserved_bytes, SOURCE_BYTES);
    drop(snapshot);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
