// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real Core admission and actual namespace loan ownership without native fakes of the reader.
use super::*;
use crate::coordinator_entry::{VerifiedEntryNamespace, fixture::Harness};
use sugarsubstitute_contracts::{
    execution::*,
    incident::{IncidentOperationId, LogicalStoreOperationKey},
    lifecycle::{CoordinatorEpoch, CoordinatorKey},
};
use sugarsubstitute_execution::ExecutionClient;
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn operation(
    namespace: sugarsubstitute_contracts::lifecycle::NamespaceId,
) -> Result<LogicalStoreOperationKey, ClaimError> {
    Ok(LogicalStoreOperationKey::new(
        CoordinatorKey {
            namespace,
            epoch: CoordinatorEpoch::new([3; 16])
                .map_err(|cause| ClaimError::new("reader_fixture_epoch", cause))?,
        },
        IncidentOperationId::new(1)
            .map_err(|cause| ClaimError::new("reader_fixture_operation", cause))?,
    ))
}
fn admitted(
    work: impl FnOnce(ExecutionClient, VerifiedEntryNamespace, JobContext) -> Result<(), ClaimError>
    + Send
    + 'static,
) -> TestResult {
    let mut harness = Harness::new()?;
    harness.discover()?;
    let namespace = harness.owner.take_namespace().ok_or("actual namespace not consumed")?;
    let main_scope =
        harness.client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let cleanup_scope =
        harness.client.create_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?;
    let mut cleanup_spec = harness.spec(Moment::from_nanos(100))?;
    cleanup_spec.operation = Operation::Infrastructure;
    cleanup_spec.deadline = None;
    cleanup_spec.reservation_bytes = 32_768;
    let mut main_spec = harness.spec(Moment::from_nanos(100))?;
    main_spec.operation = Operation::Infrastructure;
    main_spec.class = JobClass::InfrastructureOrdinary;
    main_spec.reservation_bytes = 8_159_232;
    let permit = harness
        .client
        .reserve_symbol_cleanup(&cleanup_scope, cleanup_spec, &main_scope, main_spec, |_| {
            Ok::<_, ClaimError>(())
        })
        .map_err(|reject| reject.error)?;
    let worker = harness.client.clone();
    let mut main = harness
        .client
        .submit_symbol_helper(&main_scope, main_spec, &permit, move |context| {
            work(worker, namespace, context)
        })
        .map_err(|reject| reject.error)?;
    let outcome = harness.consume_job(&mut main);
    let mut cleanup = permit.cancel_and_observe();
    let cleanup = harness.consume_job(&mut cleanup)?;
    let entry_close = harness.close(Moment::from_nanos(40_000_000_000));
    assert!(matches!(cleanup.outcome, JobOutcome::Cancelled));
    entry_close?;
    match outcome?.outcome {
        JobOutcome::Success(()) => Ok(()),
        other => Err(format!("reader actual job failed: {other:?}").into()),
    }
}
fn construct(
    client: &ExecutionClient,
    namespace: &VerifiedEntryNamespace,
    context: &JobContext,
) -> Result<CommittedDiagnosticReader, ClaimError> {
    let request = namespace
        .committed_read_request(operation(namespace.namespace())?)
        .map_err(|cause| ClaimError::new("reader_fixture_request", cause))?;
    let admission = client
        .committed_reader_admission(context)
        .map_err(|cause| ClaimError::new("reader_fixture_admission", cause))?;
    CommittedDiagnosticReader::new(request, admission).map_err(|failure| failure.cause)
}
#[test]
fn actual_reader_retains_original_three_mib_and_preheld_five_outputs_until_drop() -> TestResult {
    admitted(|client, namespace, context| {
        let owner = construct(&client, &namespace, &context)?;
        let observation = context.native_handles().ok_or_else(missing)?.observation();
        let remaining = context
            .memory()
            .claim(8_159_232 - 3_145_728 - 4096)
            .map_err(|cause| ClaimError::new("reader_fixture_remaining", cause))?;
        assert_eq!(observation.snapshot(), (5, 5));
        assert!(context.memory().claim(1).is_err());
        assert!(client.committed_reader_admission(&context).is_err());
        drop(owner);
        assert_eq!(observation.snapshot(), (0, 5));
        let returned = context
            .memory()
            .claim(3_145_728)
            .map_err(|cause| ClaimError::new("reader_fixture_return", cause))?;
        drop(returned);
        drop(remaining);
        Ok(())
    })
}
#[test]
fn namespace_loan_excludes_second_reader_and_restores_identical_state() -> TestResult {
    admitted(|client, namespace, context| {
        let owner = construct(&client, &namespace, &context)?;
        let request = namespace
            .committed_read_request(operation(namespace.namespace())?)
            .map_err(|cause| ClaimError::new("reader_fixture_request", cause))?;
        assert!(
            crate::coordinator_entry::read_namespace::NamespaceReadLoan::take(
                &request,
                owner.original_origin()
            )
            .is_err()
        );
        let original = owner.original.clone();
        drop(owner);
        let loan =
            crate::coordinator_entry::read_namespace::NamespaceReadLoan::take(&request, &original)?;
        assert_eq!(loan.resolved()?.namespace, request.namespace());
        drop(loan);
        drop(original);
        Ok(())
    })
}
#[test]
fn escaped_reader_cause_alias_keeps_original_claim_after_owner_and_control_drop() -> TestResult {
    admitted(|client, namespace, context| {
        let mut owner = construct(&client, &namespace, &context)?;
        let remaining = context
            .memory()
            .claim(8_159_232 - 3_145_728 - 4096)
            .map_err(|cause| ClaimError::new("reader_fixture_remaining", cause))?;
        let fields = owner.fields.as_mut().ok_or_else(missing)?;
        let cause = fields.causes.selected_error(
            "reader_actual_bounded_cause",
            crate::installed_release::CaptureBoundedCause::RawIo(
                std::io::Error::from_raw_os_error(5),
            ),
            false,
        );
        let alias = cause.clone();
        drop(cause);
        drop(owner);
        assert!(context.memory().claim(1).is_err());
        assert!(std::error::Error::source(&alias).is_some_and(|cause| {
            cause
                .downcast_ref::<std::io::Error>()
                .is_some_and(|cause| cause.raw_os_error() == Some(5))
        }));
        drop(alias);
        let returned = context
            .memory()
            .claim(3_145_728)
            .map_err(|cause| ClaimError::new("reader_fixture_alias_return", cause))?;
        drop(returned);
        drop(remaining);
        Ok(())
    })
}
#[test]
fn closed_namespace_revokes_reader_effects_without_releasing_existing_credits() -> TestResult {
    admitted(|client, namespace, context| {
        let owner = construct(&client, &namespace, &context)?;
        let observation = context.native_handles().ok_or_else(missing)?.observation();
        owner.check_effect().map_err(|cause| ClaimError::new("reader_fixture_current", cause))?;
        namespace.lifetime.close_admission();
        assert!(owner.check_effect().is_err());
        assert_eq!(observation.snapshot(), (5, 5));
        drop(owner);
        assert_eq!(observation.snapshot(), (0, 5));
        Ok(())
    })
}
#[test]
fn foreign_namespace_request_rejects_before_actual_core_reader_admission() -> TestResult {
    admitted(|_client, namespace, _context| {
        let other = sugarsubstitute_contracts::lifecycle::NamespaceId::new([9; 32]);
        assert!(namespace.committed_read_request(operation(other)?).is_err());
        Ok(())
    })
}
