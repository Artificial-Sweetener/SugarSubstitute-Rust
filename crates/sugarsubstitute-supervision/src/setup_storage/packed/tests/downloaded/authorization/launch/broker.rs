// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Actual accepted-payload custody survives offered, published and correlated broker outcomes.
use super::*;
use crate::{
    application_launch::{ApplicationLaunch, CreationOutcome},
    setup_storage::{ReservedLaunch, reserve_installed_launch},
};

/// Prepare through the ordinary accepted-selection owner; fixture bytes remain inert.
fn source(
    directory: &Directory,
    context: &JobContext,
) -> Result<ApplicationLaunch, Box<dyn std::error::Error + Send + Sync>> {
    let ReservedLaunch::Application(application) =
        reserve_installed_launch(&directory.path, context)?
    else {
        return Err("Accepted application absent".into());
    };
    Ok(ApplicationLaunch::selected(application.prepare_inheritance(context)?)?)
}
/// Fake only the separate creator's message boundary; source custody and validation remain real.
fn response(id: u8, disposition: &str) -> io::Result<Vec<u8>> {
    serde_json::to_vec(&serde_json::json!({"version":1,"id":vec![id;32],"disposition":disposition}))
        .map_err(io::Error::other)
}

#[test]
fn unexposed_offer_closes_local_creation_and_can_retire_without_publishing() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 96 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let directory = Directory::open(temporary.path(), false)?;
        accept_fixture(&directory, context)?;
        let launch = source(&directory, context)?;
        let offer = launch.offer([7; 32], context)?;
        assert!(launch.launch(context).is_err());
        assert!(launch.offer([8; 32], context).is_err());
        assert!(offer.observe_result(&response(7, "not_created")?, context).is_err());
        assert_eq!(launch.recover_and_retire(context)?, CreationOutcome::NotCreated);
        assert!(offer.frame(context).is_err());
        Ok(())
    })
}

#[test]
fn unanswered_publication_retains_source_slots_until_a_correlated_creator_result() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 96 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let directory = Directory::open(temporary.path(), false)?;
        let (generation, _) = accept_fixture(&directory, context)?;
        let payload = directory
            .path
            .join("releases")
            .join(generation.id.as_str())
            .join("sugarsubstitute.exe");
        let launch = source(&directory, context)?;
        let offer = launch.offer([7; 32], context)?;
        let frame: serde_json::Value = serde_json::from_slice(offer.frame(context)?)?;
        assert_eq!(frame["entry"]["kind"], "installed");
        assert!(!frame["pins"].as_array().ok_or("Pins absent")?.is_empty());
        assert!(launch.recover_and_retire(context).is_err());
        assert!(launch.retire().is_err());
        assert!(offer.observe_result(&response(8, "not_created")?, context).is_err());
        assert!(std::fs::OpenOptions::new().write(true).open(&payload).is_err());
        assert_eq!(
            offer.observe_result(&response(7, "not_created")?, context)?,
            CreationOutcome::NotCreated
        );
        assert_eq!(launch.recover_and_retire(context)?, CreationOutcome::NotCreated);
        assert!(std::fs::OpenOptions::new().write(true).open(payload).is_ok());
        Ok(())
    })
}

#[test]
fn correlated_creation_cannot_be_downgraded_by_a_duplicate_or_contradictory_reply() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 96 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let directory = Directory::open(temporary.path(), false)?;
        accept_fixture(&directory, context)?;
        let launch = source(&directory, context)?;
        let offer = launch.offer([7; 32], context)?;
        offer.frame(context)?;
        let created = response(7, "created")?;
        assert_eq!(offer.observe_result(&created, context)?, CreationOutcome::Created);
        assert_eq!(offer.observe_result(&created, context)?, CreationOutcome::Created);
        assert!(offer.observe_result(&response(7, "not_created")?, context).is_err());
        assert!(offer.frame(context).is_err());
        assert_eq!(launch.recover_and_retire(context)?, CreationOutcome::Created);
        assert_eq!(offer.observe_result(&created, context)?, CreationOutcome::Created);
        assert!(launch.launch(context).is_err());
        Ok(())
    })
}

#[test]
fn cleanup_context_cannot_publish_an_offer_after_its_original_scope_was_cancelled() -> TestResult {
    use sugarsubstitute_contracts::execution::{
        ExecutionPort, JobClass, JobOutcome, JobSpec, Moment, Operation, ScopeId, ScopeSpec,
    };
    let mut pipeline = worker::pipeline::Pipeline::new()?;
    let root = pipeline.run(TRUST_RESERVATION_BYTES + 64 * 1024 * 1024, |context| {
        let root = tempfile::tempdir()?;
        let directory = Directory::open(root.path(), false)?;
        accept_fixture(&directory, &context)?;
        Ok(root)
    })?;
    let path = root.path().to_owned();
    let launch = pipeline.run(crate::setup_storage::LAUNCH_RESERVATION_BYTES, move |context| {
        source(&Directory::open(&path, false)?, &context)
    })?;
    let owned = launch.clone();
    let offer = pipeline
        .run(crate::application_launch::broker::TRANSFER_RESERVATION_BYTES, move |context| {
            Ok(owned.offer([7; 32], &context)?)
        })?;
    let (mut job, _cleanup) = pipeline.admit(|client, original, id| {
        original.cancel();
        let cleanup =
            client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
        let job = client.submit_blocking(
            &cleanup,
            JobSpec {
                id,
                operation: Operation::NativeLifecycle,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(Moment::from_nanos(2_000_000_000)),
                reservation_bytes: crate::application_launch::broker::TRANSFER_RESERVATION_BYTES,
                replacement: None,
            },
            move |context| -> TestResult {
                assert!(!context.cancellation_requested());
                assert_eq!(
                    offer.frame(&context).err().ok_or("Cancelled offer published")?.operation(),
                    "launch_transfer_admission"
                );
                assert_eq!(launch.recover_and_retire(&context)?, CreationOutcome::NotCreated);
                Ok(())
            },
        )?;
        Ok((job, cleanup))
    })?;
    match pipeline.observe(|| job.try_take_completion())?.outcome {
        JobOutcome::Success(()) => Ok(()),
        JobOutcome::Failure(cause) => Err(cause),
        other => Err(format!("Unexpected cleanup outcome: {:?}", other.kind()).into()),
    }
}
