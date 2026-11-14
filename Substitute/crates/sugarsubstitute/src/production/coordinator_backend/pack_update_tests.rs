// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A SugarCubes update reports what it installed, however its job ended.
use super::*;
use std::sync::Arc;

/// A stop reaches the SugarCubes update while it is replacing files. The update finishes the
/// replacement before it looks at the stop, and the execution owner still ends its job as
/// cancelled. The version it installed is installed, so the window is told it updated, never
/// that the update did not finish.
#[test]
fn an_update_stopped_while_it_installs_is_reported_as_what_it_installed() -> Result<()> {
    use sugarsubstitute_contracts::update::pack::{PackOutcome, PackUpdateStatus};
    use sugarsubstitute_supervision::pack_install::{
        ReleaseSourceKind, ReleaseVersion, StartupOutcome,
    };
    let clock = Arc::new(tests::TestClock::default());
    let wake = Arc::new(crate::updates::test_support::Wake::default());
    let executor = sugarsubstitute_execution::ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = executor.client();
    let scopes = ScopeSequence::new();
    let native =
        client.create_native_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, None)?;
    let cleanup =
        client.create_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, None)?;
    let jobs = JobSequence::new();
    let mut authority = CoordinatorAuthority::new(
        sugarsubstitute_contracts::coordinator_entry::EntrySetupBudget::new(Moment::from_nanos(0))?,
        None,
    )?;
    let mut held = tests::unadmitted_policy()?;
    let backend = held.backend.as_mut();
    let resources = BackendResources {
        client: &client,
        native: &native,
        cleanup: &cleanup,
        jobs: &jobs,
        scopes: &scopes,
        authority: &mut authority,
        capture: None,
    };
    let spec = CoordinatorBackend::spec(
        &resources,
        Operation::Infrastructure,
        JobClass::InfrastructureLifecycle,
        1024 * 1024,
        Some(Moment::from_nanos(300_000_000_000)),
    )?;
    let key = ResourceKey {
        owner: tests::key()?,
        id: ResourceId::new(1)?,
        generation: ResourceGeneration::new(1)?,
    };
    let installed = StartupOutcome::Updated {
        from: ReleaseVersion::parse("0.15.3")?,
        to: ReleaseVersion::parse("0.15.4")?,
        source: ReleaseSourceKind::Registry,
        unchecked: Vec::new(),
    };
    let kept = super::pack_update::KeptOutcome::default();
    let replacing = Arc::new(crate::completion_wake::Gate::default());
    let _release = replacing.guard();
    let (job_kept, job_replacing, job_installed) =
        (kept.clone(), replacing.clone(), installed.clone());
    let job = client.submit_blocking(&cleanup, spec, move |_| {
        // The replacement runs to its end whatever is asked of the job meanwhile.
        job_replacing.hold();
        job_kept.get_or_init(|| job_installed.clone());
        Ok(super::pack_update::PackJobResult::Update(job_installed))
    })?;
    backend.pack.hold_update(key, job, cleanup.id(), spec, kept);
    replacing.wait_entered();
    backend.pack.cancel(key);
    replacing.release();
    crate::updates::test_support::observe(&wake, || {
        client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
        backend.advance_pack(&resources).map_err(|cause| -> Box<dyn Error> { cause })?;
        Ok(backend.pack_idle())
    })
    .map_err(|cause| cause.to_string())?;
    let PackUpdateStatus::Finished(report) = backend.pack_update_status() else {
        return Err("the update's result was not published".into());
    };
    assert_eq!(report.outcome, PackOutcome::Updated, "{report:?}");
    Ok(())
}
