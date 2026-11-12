// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Consume both real queue transitions without borrowing an unrelated native completion.
use super::*;
use crate::job_sequence::JobSequence;
use crate::production::incident_host::tests::constructor::report_queue::{
    prepared_host, present, recover, store_completed,
};
use std::sync::atomic::{AtomicU64, Ordering};
use sugarsubstitute_supervision::diagnostic_storage::DIAGNOSTIC_STORAGE_BYTES;
struct ClockAt(AtomicU64);
impl Clock for ClockAt {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::Acquire))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
fn activation() -> Result<Box<ProductionActivation>> {
    let mut activation = ProductionActivation::new(
        r"E:\unopened\sugarsubstitute.exe".into(),
        r"E:\unopened".into(),
        r"D:\unopened".into(),
    )?;
    activation
        .incidents
        .reporting
        .configure_locale(sugarsubstitute_contracts::configuration::LocaleId::new("en")?)?;
    Ok(activation)
}

fn context(now: Moment) -> Result<IncidentRequestContext> {
    Ok(IncidentRequestContext::new(
        IncidentOperationBudget::new(now)?,
        IncidentUtc::from_unix_seconds(110)?,
    ))
}
#[test]
fn fresh_restart_ack_queue_and_store_queue_wake_before_original_cutoff_without_unrelated_event()
-> Result<()> {
    let clock = Arc::new(ClockAt(AtomicU64::new(0)));
    let wake = Arc::new(Wake::default());
    let execution = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = execution.client();
    let finite = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let jobs = JobSequence::new();
    let (mut host, entry) = prepared_host(&client, &finite, &wake, &jobs)?;
    clock.0.store(600_000_000_000, Ordering::Release);
    let now = client.now()?;
    let fresh = sugarsubstitute_contracts::coordinator_entry::EntrySetupBudget::new(now)?;
    let original_cutoff = fresh.until();
    let mut specification = spec()?;
    specification.id = jobs.allocate()?;
    specification.deadline = Some(context(now)?.budget().until());
    specification.reservation_bytes = DIAGNOSTIC_STORAGE_BYTES;
    let mut presented =
        client.submit_blocking(&finite, specification, |context| presented_fixture(&context))?;
    let (operation, fixture, summary) = match wake.completion(&client, &mut presented)?.outcome {
        JobOutcome::Success(value) => value,
        JobOutcome::Failure(cause) => return Err(cause as Box<dyn Error>),
        _ => return Err("Actual presented repository worker did not succeed".into()),
    };
    drop(presented);
    let cleanup = client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    retire_repository(operation, &client, &cleanup, &wake, &jobs)?;
    let context = context(now)?;
    let binding = present(
        &mut host,
        fixture_original().map_err(|cause| cause as Box<dyn Error>)?,
        summary,
        context,
    )?;
    let mut activation = activation()?;
    activation.incidents.reporting.relay_until = Some(original_cutoff);
    let command = ReportCommand::new(binding, ReportAction::DismissAndRequestRestart);
    activation
        .incidents
        .reporting
        .enqueue_prepared_restart_acknowledgement(&mut host, command, context)?;
    assert!(
        matches!(host.next_effect()?, Some(IncidentEffect::Acknowledge { binding: queued, .. }) if *queued == binding)
    );
    let anchor = Instant::now();
    activation.arm_next_wake(&client, anchor)?;
    assert_eq!(activation.next_deadline(), Some(anchor));
    activation.arm_next_wake(&client, anchor)?;
    assert_eq!(
        activation.next_deadline(),
        anchor
            .checked_add(sugarsubstitute_contracts::coordinator_entry::EntrySetupBudget::DURATION),
    );
    let Some(IncidentEffect::Acknowledge { scope, binding: queued, at }) = host.take_effect()?
    else {
        return Err("Actual queued acknowledgement lost".into());
    };
    assert_eq!(queued, binding);
    // The same real effect enters StoreWork. No received/issued native frame is fabricated.
    activation.incidents.reporting.begin_store(
        &mut host,
        scope,
        binding,
        StoreKind::Acknowledge(command),
        at,
    )?;
    activation.arm_next_wake(&client, anchor)?;
    assert_eq!(activation.next_deadline(), Some(anchor));
    activation.arm_next_wake(&client, anchor)?;
    assert_eq!(activation.next_deadline(), anchor.checked_add(Duration::from_secs(60)));
    let work = activation.incidents.reporting.store.as_ref().ok_or("Actual StoreWork missing")?;
    let input = work.input.as_ref().ok_or("Actual queued input missing")?;
    assert_eq!(work.scope, scope);
    assert!(input.authentication.is_none());
    let ReportingWork::Acknowledge { command: actual, at: actual_at } = input.work else {
        return Err("Actual queued command changed".into());
    };
    assert_eq!(actual, command);
    assert_eq!(actual_at, at);
    // Exercise the actual protected repository CAS with that exact command and retained storage.
    // This filesystem-boundary proof grants no Reporter capability or native ACK authority.
    specification.id = jobs.allocate()?;
    let mut acknowledged = client.submit_blocking(&finite, specification, move |context| {
        acknowledge_fixture(fixture, scope.operation(), actual, actual_at, &context)
    })?;
    let (operation, summary) = match wake.completion(&client, &mut acknowledged)?.outcome {
        JobOutcome::Success(value) => value,
        JobOutcome::Failure(cause) => return Err(cause as Box<dyn Error>),
        _ => return Err("Actual acknowledgement worker did not succeed".into()),
    };
    retire_repository(operation, &client, &cleanup, &wake, &jobs)?;
    assert_eq!(summary.facts().retention().acknowledged(), Some(at));
    assert_eq!(fresh.until(), original_cutoff);
    assert_eq!(activation.incidents.reporting.relay_until, Some(original_cutoff));
    assert!(host.next_effect()?.is_none());
    drop(acknowledged);
    drop(activation);
    drop(host);
    drop(entry);
    drop(execution);
    Ok(())
}
#[test]
fn store_continuation_clears_while_actual_transaction_blocks_without_renewing_deadline()
-> Result<()> {
    let wake = Arc::new(Wake::default());
    let execution = ExecutionOwner::new(Arc::new(ClockAt(AtomicU64::new(0))), wake.clone())?;
    let client = execution.client();
    let finite = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let cleanup = client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let jobs = JobSequence::new();
    let (mut host, entry) = prepared_host(&client, &finite, &wake, &jobs)?;
    let mut activation = activation()?;
    let original = fixture_original().map_err(|cause| cause as Box<dyn Error>)?;
    let binding = recover(&mut host, original, context(client.now()?)?)?;
    let scope = host.admit_report_view(binding, context(client.now()?)?)?;
    activation.incidents.reporting.begin_store(
        &mut host,
        scope,
        binding,
        StoreKind::View { previous: None },
        context(client.now()?)?.at(),
    )?;
    let mut transactions = DiagnosticTransactions::bound_fixture(scope.operation().owner())?;
    let mut competing = None;
    let competing_operation = LogicalStoreOperationKey::new(
        scope.operation().owner(),
        IncidentOperationId::new(scope.operation().id().get() + 1)?,
    );
    assert!(transactions.admit(
        DiagnosticTransactionRole::RecoveryOrigin,
        competing_operation,
        &mut competing
    )?);
    let anchor = Instant::now();
    activation.arm_next_wake(&client, anchor)?;
    assert_eq!(activation.next_deadline(), Some(anchor));
    activation.incidents.reporting.advance_store(
        &mut host,
        &client,
        &finite,
        &cleanup,
        &jobs,
        &mut transactions,
    )?;
    assert!(
        activation
            .incidents
            .reporting
            .store
            .as_ref()
            .is_some_and(|work| !work.installed && !work.submitted && work.input.is_some())
    );
    activation.arm_next_wake(&client, anchor)?;
    assert_eq!(activation.next_deadline(), anchor.checked_add(Duration::from_secs(60)));
    transactions.retired(competing_operation, &mut competing)?;
    activation.incidents.reporting.advance_store(
        &mut host,
        &client,
        &finite,
        &cleanup,
        &jobs,
        &mut transactions,
    )?;
    let work = activation.incidents.reporting.store.as_ref().ok_or("Submitted StoreWork lost")?;
    assert!(work.installed && work.submitted && work.input.is_none());
    assert_eq!(work.scope.original(), scope.original());
    activation.arm_next_wake(&client, anchor)?;
    assert_eq!(activation.next_deadline(), anchor.checked_add(Duration::from_secs(60)));
    store_completed(&mut host, &client, &wake, false)?;
    let cleanup_until = client.now()?.checked_add(Duration::from_secs(15))?;
    host.reporting_service()?.submit_cleanup(&client, &cleanup, &jobs, cleanup_until)?;
    store_completed(&mut host, &client, &wake, true)?;
    assert!(
        host.reporting_service()?.files_retired(),
        "Actual store cleanup failed: {:?}",
        host.reporting_service()?
            .inspect(|operation| format!("{:?}", operation.fixture_retirement_cause()))
    );
    drop(activation);
    drop(host);
    drop(entry);
    drop(execution);
    Ok(())
}

fn retire_repository(
    operation: ReportingOperation,
    client: &ExecutionClient,
    cleanup: &Scope,
    wake: &Wake,
    jobs: &JobSequence,
) -> Result<()> {
    let cleanup_until = client.now()?.checked_add(Duration::from_secs(15))?;
    let mut specification = spec()?;
    specification.id = jobs.allocate()?;
    specification.deadline = Some(cleanup_until);
    specification.reservation_bytes = 32 * 1024;
    let mut retiring = client.submit_blocking(cleanup, specification, move |context| {
        Ok::<_, std::io::Error>(operation.retire_fixture(&context))
    })?;
    let (retired, operation) = match wake.completion(client, &mut retiring)?.outcome {
        JobOutcome::Success(value) => value,
        JobOutcome::Failure(cause) => return Err(cause.into()),
        _ => return Err("Actual independent repository cleanup worker did not succeed".into()),
    };
    assert!(
        retired,
        "Actual repository cleanup rejected: {:?}",
        operation.fixture_retirement_cause()
    );
    assert!(operation.files_retired());
    assert_eq!(retiring.spec().deadline, Some(cleanup_until));
    drop(operation);
    drop(retiring);
    Ok(())
}
