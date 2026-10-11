// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Use real registry/execution policy with a private external stdio artifact fake.
use super::*;
use crate::RegistryInputError;
use crate::registry_input::output::{
    OutputLaunch, OutputPin, association_error, binding_matches,
    fake::{Pin, Source},
};
use sugarsubstitute_contracts::lifecycle::{ResourceGeneration, ResourceId, ResourceKey};
fn resource() -> Result<ResourceKey, Box<dyn Error>> {
    Ok(ResourceKey {
        owner: owner()?,
        id: ResourceId::new(1)?,
        generation: ResourceGeneration::new(1)?,
    })
}
fn captured(
    binding: FamilyBinding,
    source: &Arc<Source>,
) -> Result<WindowsLaunchInput, Box<dyn Error>> {
    let mut input = input()?;
    input.output = Some(OutputLaunch {
        binding,
        resource: source.resource,
        stdio: OutputPin::Simulated(Pin(source.clone())),
    });
    Ok(input)
}
fn submit(
    h: &mut Harness,
    binding: FamilyBinding,
    input: WindowsLaunchInput,
) -> Result<LaunchTicket, RegistryAdmissionError> {
    let watch = h
        .spec(JobClass::NativeObservation, 4096)
        .map_err(|_| RegistryAdmissionError::InvalidSpecification)?;
    let creation = h
        .spec(JobClass::InfrastructureLifecycle, input.required_reservation_bytes())
        .map_err(|_| RegistryAdmissionError::InvalidSpecification)?;
    let scope = ScopeSpec {
        id: ScopeId::new(h.next_scope).map_err(|_| RegistryAdmissionError::InvalidSpecification)?,
        deadline: None,
    };
    h.next_scope += 1;
    h.registry.submit_launch(&h.client, &h.scope, scope, creation, watch, binding, input)
}
#[test]
fn managed_output_requires_exact_owner_and_resource_generation() -> TestResult {
    let current = resource()?;
    assert!(binding_matches(FamilyBinding::Managed(current), current));
    let mut stale = current;
    stale.generation = stale.generation.next()?;
    assert!(!binding_matches(FamilyBinding::Managed(current), stale));
    let mut foreign = current;
    foreign.owner.epoch = CoordinatorEpoch::new([2; 16])?;
    assert!(!binding_matches(binding(1)?, foreign));
    assert!(binding_matches(binding(1)?, current));
    Ok(())
}
#[test]
fn association_rejects_foreign_session_and_stale_resource_without_touching_sources() -> TestResult {
    let current = resource()?;
    let launch = input()?;
    let source = Source::new(current);
    let foreign = Source::new(current);
    assert!(association_error(&launch, binding(1)?, current, current, current, true).is_none());
    assert!(matches!(
        association_error(&launch, binding(1)?, current, current, current, false),
        Some(RegistryInputError::OutputProvenance)
    ));
    let mut stale = current;
    stale.generation = stale.generation.next()?;
    assert!(matches!(
        association_error(&launch, binding(1)?, current, stale, current, true),
        Some(RegistryInputError::OutputProvenance)
    ));
    assert!(matches!(
        association_error(&launch, FamilyBinding::Managed(stale), current, current, current, true),
        Some(RegistryInputError::OutputBinding)
    ));
    assert_eq!(source.requests.load(Ordering::Acquire), 0);
    assert_eq!(foreign.requests.load(Ordering::Acquire), 0);
    let occupied = captured(binding(1)?, &source)?;
    assert!(matches!(
        association_error(&occupied, binding(1)?, current, current, current, true),
        Some(RegistryInputError::OutputConflict)
    ));
    assert_eq!(source.requests.load(Ordering::Acquire), 0);
    Ok(())
}
#[test]
fn rejected_creation_admission_cancels_owned_output_and_observes_unbound_watch_retirement()
-> TestResult {
    let mut h = Harness::new()?;
    let source = Source::new(resource()?);
    h.scope.cancel();
    assert!(matches!(
        submit(&mut h, binding(1)?, captured(binding(1)?, &source)?),
        Err(RegistryAdmissionError::Execution(_))
    ));
    assert_eq!(h.fake.creates.load(Ordering::Acquire), 0);
    assert_eq!(source.requests.load(Ordering::Acquire), 1);
    assert_eq!(source.releases.load(Ordering::Acquire), 1);
    h.drain_until(|registry| registry.records.iter().all(Option::is_none))?;
    Ok(())
}
#[test]
fn dropping_accepted_unsubmitted_input_requests_only_its_owned_output() -> TestResult {
    let source = Source::new(resource()?);
    let other = Source::new(resource()?);
    drop(captured(binding(1)?, &source)?);
    assert_eq!(source.requests.load(Ordering::Acquire), 1);
    assert_eq!(source.releases.load(Ordering::Acquire), 1);
    assert_eq!(other.requests.load(Ordering::Acquire), 0);
    Ok(())
}
#[test]
fn changed_binding_rejects_before_watch_admission_and_native_effects() -> TestResult {
    let mut h = Harness::new()?;
    let source = Source::new(resource()?);
    assert!(matches!(
        submit(&mut h, binding(2)?, captured(binding(1)?, &source)?),
        Err(RegistryAdmissionError::InvalidSpecification)
    ));
    assert_eq!(h.fake.creates.load(Ordering::Acquire), 0);
    assert!(h.registry.records.iter().all(Option::is_none));
    assert_eq!(source.requests.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn standalone_output_sets_only_three_std_handles_and_success_does_not_cancel() -> TestResult {
    let mut h = Harness::new()?;
    let source = Source::new(resource()?);
    let key = submit(&mut h, binding(1)?, captured(binding(1)?, &source)?)?.process();
    h.created(key)?;
    assert_eq!(*lock(&h.fake.inherited), ([71, 72, 73, 0], 3, [71, 72, 73], true));
    assert_eq!(source.releases.load(Ordering::Acquire), 1);
    assert_eq!(source.requests.load(Ordering::Acquire), 0);
    h.fake.root_alive.store(false, Ordering::Release);
    h.exit(key, ROOT, true, 0)?;
    h.stop(key)?;
    h.cleanup(key)?;
    assert!(h.registry.snapshot(key)?.settled);
    assert_eq!(source.requests.load(Ordering::Acquire), 0);
    Ok(())
}
#[test]
fn cancellation_during_partial_creation_retains_pin_until_actual_attribute_deletion() -> TestResult
{
    let mut h = Harness::new()?;
    let source = Source::new(resource()?);
    h.fake.hold_create.store(true, Ordering::Release);
    let key = submit(&mut h, binding(1)?, captured(binding(1)?, &source)?)?.process();
    h.fake.entered()?;
    assert_eq!(source.releases.load(Ordering::Acquire), 0);
    h.scope.cancel();
    h.fake.release();
    h.created(key)?;
    assert_eq!(source.releases.load(Ordering::Acquire), 0);
    {
        let index = h.registry.index(key)?;
        let mut state =
            lock(&h.registry.records[index].as_ref().ok_or("Missing family")?.shared.native);
        assert!(
            state.family.as_mut().ok_or("Missing native family")?.release_inheritance().is_err()
        );
    }
    assert_eq!(source.releases.load(Ordering::Acquire), 0);
    assert!(source.requests.load(Ordering::Acquire) > 0);
    assert_eq!(h.fake.resumes.load(Ordering::Acquire), 0);
    drop(h);
    assert_eq!(source.releases.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn creation_panic_preserves_original_fatal_cause_and_cancels_the_adopted_source() -> TestResult {
    let mut h = Harness::new()?;
    let source = Source::new(resource()?);
    h.fake.panic_create.store(true, Ordering::Release);
    let key = submit(&mut h, binding(1)?, captured(binding(1)?, &source)?)?.process();
    h.created(key)?;
    assert!(source.requests.load(Ordering::Acquire) > 0);
    assert_eq!(source.releases.load(Ordering::Acquire), 0);
    let index = h.registry.index(key)?;
    assert!(
        h.registry.records[index]
            .as_ref()
            .and_then(|r| r.error.as_ref())
            .is_some_and(RegistryError::contains_owner_panic)
    );
    drop(h);
    assert_eq!(source.releases.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn failed_post_resume_close_keeps_pin_and_started_fact_until_checked_cleanup() -> TestResult {
    let mut h = Harness::new()?;
    let source = Source::new(resource()?);
    h.fake.fail_resume_close.store(true, Ordering::Release);
    let key = submit(&mut h, binding(1)?, captured(binding(1)?, &source)?)?.process();
    h.created(key)?;
    assert!(h.registry.snapshot(key)?.started);
    assert!(source.requests.load(Ordering::Acquire) > 0);
    assert_eq!(source.releases.load(Ordering::Acquire), 0);
    drop(h);
    assert_eq!(source.releases.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn output_closed_during_native_creation_never_reaches_resume() -> TestResult {
    let mut h = Harness::new()?;
    let source = Source::new(resource()?);
    h.fake.hold_create.store(true, Ordering::Release);
    let key = submit(&mut h, binding(1)?, captured(binding(1)?, &source)?)?.process();
    h.fake.entered()?;
    source.closed.store(true, Ordering::Release);
    h.fake.release();
    h.created(key)?;
    assert_eq!(h.fake.resumes.load(Ordering::Acquire), 0);
    assert!(!h.registry.snapshot(key)?.started);
    assert_eq!(source.releases.load(Ordering::Acquire), 0);
    assert!(source.requests.load(Ordering::Acquire) > 0);
    Ok(())
}
struct BlockedWorkers {
    state: Mutex<(usize, bool)>,
    changed: Condvar,
}
impl BlockedWorkers {
    fn hold(&self) -> Result<(), RegistryError> {
        let mut state = lock(&self.state);
        state.0 += 1;
        self.changed.notify_all();
        let (_state, timed) = self
            .changed
            .wait_timeout_while(state, Duration::from_secs(10), |state| !state.1)
            .map_err(|_| RegistryError::invalid("blocked_worker_poison"))?;
        if timed.timed_out() {
            return Err(RegistryError::invalid("blocked_worker_not_released"));
        }
        Ok(())
    }
    fn entered(&self) -> TestResult {
        let (_state, timed) = self
            .changed
            .wait_timeout_while(lock(&self.state), Duration::from_secs(10), |state| state.0 != 2)
            .map_err(|_| "Blocked worker fixture poisoned")?;
        if timed.timed_out() {
            return Err("Both finite workers did not enter".into());
        }
        Ok(())
    }
    fn release(&self) {
        lock(&self.state).1 = true;
        self.changed.notify_all();
    }
}
struct ReleaseWorkers(Arc<BlockedWorkers>);
impl Drop for ReleaseWorkers {
    fn drop(&mut self) {
        self.0.release();
    }
}
#[test]
fn queued_creation_cancellation_drops_accepted_pin_without_entering_native_boundary() -> TestResult
{
    use sugarsubstitute_contracts::execution::ExecutionPort;
    let mut h = Harness::new()?;
    let source = Source::new(resource()?);
    let blockers =
        Arc::new(BlockedWorkers { state: Mutex::new((0, false)), changed: Condvar::new() });
    let release = ReleaseWorkers(blockers.clone());
    let mut jobs = Vec::new();
    for _ in 0..2 {
        let spec = h.spec(JobClass::InfrastructureLifecycle, 4096)?;
        let boundary = blockers.clone();
        jobs.push(h.client.submit_blocking(&h.scope, spec, move |_| boundary.hold())?);
    }
    blockers.entered()?;
    let key = submit(&mut h, binding(1)?, captured(binding(1)?, &source)?)?.process();
    h.scope.cancel();
    drop(release);
    h.created(key)?;
    assert_eq!(h.fake.creates.load(Ordering::Acquire), 0);
    assert_eq!(source.requests.load(Ordering::Acquire), 1);
    assert_eq!(source.releases.load(Ordering::Acquire), 1);
    drop(jobs);
    Ok(())
}
