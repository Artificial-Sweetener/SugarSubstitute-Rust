// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise diagnostic family ownership using genuine admitted transport pins and external native fakes.
use super::*;
use std::os::windows::io::AsRawHandle;
use sugarsubstitute_contracts::incident::{DiagnosticKey, DiagnosticRole, DiagnosticSessionId};
use sugarsubstitute_execution::windows_diagnostic_pipe::{
    DiagnosticEnrollment, DiagnosticPipeBootstrap, DiagnosticPipeObservation,
    DiagnosticPipeSession, MIN_LIFECYCLE_PIPE_BYTES,
};
/// Give concurrent actual Windows servers distinct process-owned session names.
fn key(role: DiagnosticRole) -> Result<DiagnosticKey, Box<dyn Error>> {
    static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);
    let sequence = NEXT_SESSION
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
            value.checked_add(1).filter(|next| *next <= u64::from(u32::MAX))
        })
        .map_err(|_| "Diagnostic fixture session identities exhausted")?;
    let session = (u64::from(std::process::id()) << 32) | sequence;
    Ok(DiagnosticKey::new(owner()?, role, DiagnosticSessionId::new(session)?))
}
/// Retain the actual native scope through all original inherited/session custody.
struct PreparedSession {
    session: DiagnosticPipeSession,
    _scope: Arc<Scope>,
}
/// Retain one actual transport scope independently of the 131 contained-family scopes.
fn pipe_scope(h: &mut Harness) -> Result<Arc<Scope>, Box<dyn Error>> {
    let scope = h
        .client
        .create_native_scope(ScopeSpec { id: ScopeId::new(h.next_scope)?, deadline: None }, None)?;
    h.next_scope += 1;
    Ok(Arc::new(scope))
}
/// Take original pins only after actual Prepared, using the existing owner/wake/drain.
fn pins(
    h: &mut Harness,
    key: DiagnosticKey,
    scope: &Arc<Scope>,
) -> Result<(PreparedSession, DiagnosticPipeBootstrap, DiagnosticEnrollment), Box<dyn Error>> {
    let spec = h.spec(JobClass::NativeObservation, MIN_LIFECYCLE_PIPE_BYTES)?;
    let mut session = h.client.reserve_diagnostic_pipe_server(
        scope,
        spec,
        key,
        Moment::from_nanos(60_000_000_000),
    )?;
    loop {
        let prior = *lock(&h.wake.count);
        h.registry.consume(&h.client.drain_completions(std::num::NonZeroUsize::MIN)?)?;
        if session
            .drain(std::num::NonZeroUsize::MIN)
            .into_iter()
            .any(|fact| matches!(fact, DiagnosticPipeObservation::Prepared))
        {
            let (bootstrap, enrollment) =
                session.take_bootstrap()?.ok_or("Actual Prepared lacks original pins")?;
            return Ok((PreparedSession { session, _scope: scope.clone() }, bootstrap, enrollment));
        }
        if let Some(completion) = session.take_terminal() {
            return match completion.outcome {
                sugarsubstitute_contracts::execution::JobOutcome::Failure(cause) => {
                    Err(Box::new(cause))
                }
                sugarsubstitute_contracts::execution::JobOutcome::Panicked(cause) => {
                    Err(Box::new(RegistryError::owner_panic("diagnostic_fixture_prepare", cause)))
                }
                _ => Err("Actual diagnostic preparation ended before Prepared".into()),
            };
        }
        h.wake.wait(prior)?;
    }
}
fn assembled(
    bootstrap: DiagnosticPipeBootstrap,
    enrollment: DiagnosticEnrollment,
) -> Result<Option<WindowsLaunchInput>, Box<dyn Error>> {
    let mut value = input()?;
    let mut bootstrap = Some(bootstrap);
    let mut enrollment = Some(enrollment);
    value.with_diagnostic_pipe(&mut bootstrap, &mut enrollment)?;
    assert!(bootstrap.is_none() && enrollment.is_none());
    Ok(Some(value))
}
fn submit(
    h: &mut Harness,
    key: DiagnosticKey,
    input: &mut Option<WindowsLaunchInput>,
) -> Result<LaunchTicket, RegistryAdmissionError> {
    let required = input
        .as_ref()
        .ok_or(RegistryAdmissionError::InvalidSpecification)?
        .required_reservation_bytes();
    let watch = h
        .spec(JobClass::NativeObservation, 4096)
        .map_err(|_| RegistryAdmissionError::InvalidSpecification)?;
    let create = h
        .spec(JobClass::InfrastructureLifecycle, required)
        .map_err(|_| RegistryAdmissionError::InvalidSpecification)?;
    let scope = ScopeSpec {
        id: ScopeId::new(h.next_scope).map_err(|_| RegistryAdmissionError::InvalidSpecification)?,
        deadline: None,
    };
    h.next_scope += 1;
    h.registry.submit_diagnostic_launch(&h.client, &h.scope, scope, create, watch, key, input)
}
/// Release only the external enrollment hold before the actual executor's fixture teardown.
struct Release(Arc<Fake>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.release();
    }
}
#[test]
fn foreign_actual_session_rejection_preserves_both_original_pins() -> TestResult {
    let mut h = Harness::new()?;
    let scope = pipe_scope(&mut h)?;
    let first_key = key(DiagnosticRole::Storage)?;
    let second_key = key(DiagnosticRole::Storage)?;
    assert_ne!(first_key, second_key);
    let (_first, a_boot, a_enroll) = pins(&mut h, first_key, &scope)?;
    let (_second, b_boot, b_enroll) = pins(&mut h, second_key, &scope)?;
    let a_raw = a_boot.borrow_inheritance().section.as_raw_handle();
    let mut bootstrap = Some(a_boot);
    let mut enrollment = Some(b_enroll);
    let mut value = input()?;
    assert!(matches!(
        value.with_diagnostic_pipe(&mut bootstrap, &mut enrollment),
        Err(crate::RegistryInputError::DiagnosticProvenance)
    ));
    assert_eq!(
        bootstrap.as_ref().ok_or("Original pin lost")?.borrow_inheritance().section.as_raw_handle(),
        a_raw
    );
    assert!(b_boot.matches_enrollment(enrollment.as_ref().ok_or("Original enrollment lost")?));
    assert!(bootstrap.as_ref().ok_or("Original pin lost")?.matches_enrollment(&a_enroll));
    assert!(value.diagnostic.is_none());
    assert_eq!(h.fake.creates.load(Ordering::Acquire), 0);
    Ok(())
}
#[test]
fn diagnostic_creation_rejection_returns_complete_original_input_and_observes_reserved_watch()
-> TestResult {
    let mut h = Harness::new()?;
    let scope = pipe_scope(&mut h)?;
    let key = key(DiagnosticRole::Storage)?;
    let (_session, bootstrap, enrollment) = pins(&mut h, key, &scope)?;
    let raw = bootstrap.borrow_inheritance().section.as_raw_handle();
    let mut input = assembled(bootstrap, enrollment)?;
    h.scope.cancel();
    assert!(matches!(submit(&mut h, key, &mut input), Err(RegistryAdmissionError::Execution(_))));
    let launch =
        input.as_ref().and_then(|value| value.diagnostic.as_ref()).ok_or("Original input lost")?;
    assert_eq!(launch.bootstrap.borrow_inheritance().section.as_raw_handle(), raw);
    assert!(launch.validates(key));
    assert_eq!(h.fake.creates.load(Ordering::Acquire), 0);
    h.drain_until(WindowsProcessRegistry::is_empty)?;
    assert!(input.is_some());
    Ok(())
}
#[test]
fn held_diagnostic_enrollment_retains_original_section_and_stop_fences_resume() -> TestResult {
    let mut h = Harness::new()?;
    let scope = pipe_scope(&mut h)?;
    let release = Release(h.fake.clone());
    let key = key(DiagnosticRole::Symbolize)?;
    let (session, bootstrap, enrollment) = pins(&mut h, key, &scope)?;
    let raw = bootstrap.borrow_inheritance().section.as_raw_handle() as usize;
    h.fake.hold_enrollment.store(true, Ordering::Release);
    let mut input = assembled(bootstrap, enrollment)?;
    let process = submit(&mut h, key, &mut input)?.process();
    assert!(input.is_none());
    h.fake.entered_enrollment()?;
    assert_eq!(*lock(&h.fake.creation_order), [1, 2, 3, 0]);
    let inherited = *lock(&h.fake.inherited);
    assert_eq!(inherited.1, 1);
    assert_eq!(inherited.0[0], raw);
    assert!(!inherited.3);
    assert!(h.scope.snapshot().reserved_bytes >= crate::registry_input::FAMILY_STORAGE_BYTES);
    h.stop(process)?;
    h.fake.release();
    h.created(process)?;
    assert_eq!(h.fake.resumes.load(Ordering::Acquire), 0);
    assert!(!h.registry.snapshot(process)?.started);
    assert!(!h.registry.snapshot(process)?.settled);
    let shared = h.registry.records[h.registry.index(process)?]
        .as_ref()
        .ok_or("Missing family")?
        .shared
        .clone();
    {
        let state = lock(&shared.native);
        let family = state.family.as_ref().ok_or("Lost contained family")?;
        assert!(
            family
                .diagnostic_bootstrap
                .as_ref()
                .zip(family.diagnostic_enrollment.as_ref())
                .is_some_and(|(a, b)| a.matches_enrollment(b))
        );
    }
    h.fake.root_alive.store(false, Ordering::Release);
    h.exit(process, ROOT, true, 0)?;
    h.cleanup(process)?;
    session.session.control().close();
    drop(shared);
    drop(session);
    assert!(h.registry.snapshot(process)?.settled);
    drop(release);
    Ok(())
}
#[test]
fn diagnostic_three_roles_and_uncertain_predecessor_preserve_separate_desktop_managed_quotas()
-> TestResult {
    use sugarsubstitute_contracts::lifecycle::{ResourceGeneration, ResourceId, ResourceKey};
    let mut h = Harness::new()?;
    let scope = pipe_scope(&mut h)?;
    let mut sessions = Vec::new();
    for (i, role) in [DiagnosticRole::Storage, DiagnosticRole::Symbolize, DiagnosticRole::Reporter]
        .into_iter()
        .enumerate()
    {
        let binding = key(role)?;
        let (session, bootstrap, enrollment) = pins(&mut h, binding, &scope)?;
        let mut input = assembled(bootstrap, enrollment)?;
        h.fake.fail_enrollment.store(i == 0, Ordering::Release);
        let process = submit(&mut h, binding, &mut input)?.process();
        h.created(process)?;
        assert_eq!(h.registry.snapshot(process)?.binding, FamilyBinding::Diagnostic(binding));
        if i == 0 {
            assert!(!h.registry.snapshot(process)?.started);
            assert!(!h.registry.snapshot(process)?.settled);
        }
        sessions.push(session);
        if i == 0 {
            let successor = key(role)?;
            let (session, bootstrap, enrollment) = pins(&mut h, successor, &scope)?;
            let mut input = assembled(bootstrap, enrollment)?;
            let effects = h.fake.creates.load(Ordering::Acquire);
            assert!(matches!(
                submit(&mut h, successor, &mut input),
                Err(RegistryAdmissionError::Busy)
            ));
            assert!(input.is_some());
            assert_eq!(h.fake.creates.load(Ordering::Acquire), effects);
            drop(input);
            sessions.push(session);
        }
    }
    h.fake.fail_enrollment.store(false, Ordering::Release);
    for id in 1..=64 {
        let process = h.launch(id)?.process();
        h.created(process)?;
    }
    for id in 1..=64 {
        let input = input()?;
        let watch = h.spec(JobClass::NativeObservation, 4096)?;
        let create =
            h.spec(JobClass::InfrastructureLifecycle, input.required_reservation_bytes())?;
        let native = ScopeSpec { id: ScopeId::new(h.next_scope)?, deadline: None };
        h.next_scope += 1;
        let binding = FamilyBinding::Managed(ResourceKey {
            owner: owner()?,
            id: ResourceId::new(id)?,
            generation: ResourceGeneration::new(1)?,
        });
        let process = h
            .registry
            .submit_launch(&h.client, &h.scope, native, create, watch, binding, input)?
            .process();
        h.created(process)?;
    }
    assert_eq!(h.registry.records.iter().flatten().count(), 131);
    assert_eq!(h.fake.creates.load(Ordering::Acquire), 131);
    assert!(h.launch(65).is_err());
    drop(sessions);
    Ok(())
}
