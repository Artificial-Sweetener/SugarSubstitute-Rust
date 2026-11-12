// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real signed image custody follows the registry; only process effects use the native boundary fake.
use super::*;
use crate::setup_storage::{
    self, InstallationRun, LAUNCH_RESERVATION_BYTES, SIGNED_INSTALLER_RESERVATION_BYTES,
    SelectionStore,
    test_package::{
        maintenance::{Fixture, accept_pending},
        repository::SignedPackage,
    },
};
use std::path::Path;
use sugarsubstitute_contracts::execution::{JobContext, JobOutcome};
use sugarsubstitute_update::fixture::NOW;

type Failure = Box<dyn Error + Send + Sync>;
mod image_tests;
mod probe_tests;
mod storage;
use storage::run as storage;

/// Create a real accepted inert package, then retain only independently verified helper files.
fn preparation(
    h: &mut Harness,
) -> Result<(Fixture, WindowsLaunchInput, std::path::PathBuf), Box<dyn Error>> {
    preparation_with(
        h,
        Some(sugarsubstitute_contracts::application_launch::updater::CURRENT_UPDATER_CONTRACT),
        None,
    )
}

fn preparation_with(
    h: &mut Harness,
    updater: Option<&'static [u8]>,
    attempt: Option<crate::UpdateAttemptPins>,
) -> Result<(Fixture, WindowsLaunchInput, std::path::PathBuf), Box<dyn Error>> {
    let fixture = storage(h, SIGNED_INSTALLER_RESERVATION_BYTES, move |context| {
        let fixture = Fixture::with_distribution(
            setup_storage::test_package::distribution_with_roles(
                Some(sugarsubstitute_contracts::application_launch::CURRENT_CONTRACT),
                None,
                updater,
            )?,
            &context,
        )?;
        let prepared = fixture
            .import(SignedPackage::new(&fixture.archive, 1)?, &context)?
            .prepare(|| NOW, &context)?;
        let runtime = prepared.maintenance_runtime().ok_or("Runtime absent")?.clone();
        drop(prepared.complete(&runtime, || NOW, &context)?);
        accept_pending(fixture.installation.path(), &context)?;
        Ok(fixture)
    })?;
    let (fixture, running) = storage(h, LAUNCH_RESERVATION_BYTES, move |context| {
        let store = SelectionStore::open(fixture.installation.path(), &context)?;
        let generation = store
            .load(&context)?
            .ok_or("Selection absent")?
            .accepted()
            .cloned()
            .ok_or("Accepted generation absent")?;
        drop(store);
        let directory = fixture.installation.path().join("releases").join(generation.id.as_str());
        let running = setup_storage::retain_running_generation(&directory, None, &context)?
            .ok_or("Running generation absent")?;
        Ok((fixture, running))
    })?;
    let owner = owner()?;
    storage(h, LAUNCH_RESERVATION_BYTES, move |context| {
        let image = running.pin_preparation(&context)?;
        let executable = image.executable();
        drop(running);
        let input = WindowsLaunchInput::preparation(
            owner,
            image,
            fixture.workspace.path(),
            crate::LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?,
            attempt,
        )?;
        assert!(!input.is_updater_probe());
        Ok((fixture, input, executable))
    })
}

fn submit(
    h: &mut Harness,
    binding: FamilyBinding,
    input: WindowsLaunchInput,
) -> Result<LaunchTicket, RegistryAdmissionError> {
    let watch = h
        .spec(JobClass::NativeObservation, 4096)
        .map_err(|_| RegistryAdmissionError::InvalidSpecification)?;
    let create = h
        .spec(JobClass::InfrastructureLifecycle, input.required_reservation_bytes())
        .map_err(|_| RegistryAdmissionError::InvalidSpecification)?;
    let native = ScopeSpec {
        id: ScopeId::new(h.next_scope).map_err(|_| RegistryAdmissionError::InvalidSpecification)?,
        deadline: None,
    };
    h.next_scope += 1;
    h.registry.submit_launch(&h.client, &h.scope, native, create, watch, binding, input)
}

fn writable(path: &Path) -> bool {
    std::fs::OpenOptions::new().write(true).open(path).is_ok()
}

#[test]
fn preparation_retains_files_through_partial_native_close_without_retaining_run() -> TestResult {
    let mut h = Harness::new()?;
    let (fixture, input, executable) = preparation(&mut h)?;
    assert_eq!(executable.file_name(), Some(std::ffi::OsStr::new("sugarsubstitute-updater.exe")));
    let text =
        String::from_utf16(&input.command.command_line[..input.command.command_line.len() - 1])?;
    assert!(
        text.contains(sugarsubstitute_contracts::application_launch::updater::PREPARATION_ARGUMENT)
    );
    assert!(text.ends_with(&format!("\"{}\"", fixture.workspace.path().display())));
    let ticket = submit(&mut h, FamilyBinding::Preparation(owner()?), input)?;
    h.created(ticket.process())?;
    assert!(h.registry.snapshot(ticket.process())?.started);
    assert!(!writable(&executable));
    assert_eq!(lock(&h.fake.inherited).1, 0);
    assert!(!h.fake.enrollment_entered.load(Ordering::Acquire));
    let fixture = storage(&mut h, setup_storage::RESERVATION_BYTES, move |context| {
        let store = Arc::new(SelectionStore::open(fixture.installation.path(), &context)?);
        drop(InstallationRun::acquire(store, &context)?);
        fixture.preserved()?;
        Ok(fixture)
    })?;
    h.fake.root_alive.store(false, Ordering::Release);
    h.exit(ticket.process(), ROOT, true, 0)?;
    h.stop(ticket.process())?;
    h.fake.fail_close_after_job.store(true, Ordering::Release);
    h.cleanup(ticket.process())?;
    assert!(!h.registry.snapshot(ticket.process())?.settled);
    assert!(!writable(&executable));
    h.cleanup(ticket.process())?;
    assert!(h.registry.snapshot(ticket.process())?.settled);
    assert!(writable(&executable));
    fixture.preserved().map_err(|cause| cause as Box<dyn Error>)?;
    Ok(())
}

/// Give concurrent test processes distinct pipe endpoints, as the diagnostic fixtures do.
fn attempt_channel() -> Result<PurposeKey, Box<dyn Error>> {
    use sugarsubstitute_contracts::update::attempt::codec::{
        UpdateAttemptChannel, UpdateAttemptKey,
    };
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let sequence = NEXT
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
            value.checked_add(1).filter(|next| *next <= u64::from(u32::MAX))
        })
        .map_err(|_| "Attempt fixture identities exhausted")?;
    let attempt = (u64::from(std::process::id()) << 32) | sequence;
    let attempt = std::num::NonZeroU64::new(attempt).ok_or("Zero attempt")?;
    Ok(PurposeKey::UpdateAttempt(UpdateAttemptChannel::new(
        owner()?,
        UpdateAttemptKey::new(attempt),
    )))
}

#[test]
fn preparation_inherits_its_update_attempt_channel_and_enrolls_the_suspended_helper() -> TestResult
{
    let mut h = Harness::new()?;
    let scope = super::diagnostic::pipe_scope(&mut h)?;
    let channel = attempt_channel()?;
    let (session, bootstrap, enrollment) =
        super::diagnostic::purpose_pins(&mut h, channel, &scope)?;
    let pins = crate::UpdateAttemptPins { bootstrap, enrollment };
    let (fixture, input, _executable) = preparation_with(
        &mut h,
        Some(sugarsubstitute_contracts::application_launch::updater::CURRENT_UPDATER_CONTRACT),
        Some(pins),
    )?;
    let text =
        String::from_utf16(&input.command.command_line[..input.command.command_line.len() - 1])?;
    assert!(text.contains(
        sugarsubstitute_contracts::application_launch::updater::preparation::ATTEMPT_ARGUMENT
    ));
    assert!(input.purpose.as_ref().is_some_and(|launch| launch.key == channel));
    let mut foreign = owner()?;
    foreign.epoch = CoordinatorEpoch::new([2; 16])?;
    assert!(matches!(
        h.registry.validate_input(FamilyBinding::Preparation(foreign), &input),
        Err(RegistryAdmissionError::InvalidSpecification)
    ));
    let ticket = submit(&mut h, FamilyBinding::Preparation(owner()?), input)?;
    h.created(ticket.process())?;
    assert!(h.fake.enrollment_entered.load(Ordering::Acquire));
    assert_eq!(lock(&h.fake.inherited).1, 1);
    h.fake.root_alive.store(false, Ordering::Release);
    h.exit(ticket.process(), ROOT, true, 0)?;
    h.stop(ticket.process())?;
    h.cleanup(ticket.process())?;
    assert!(h.registry.snapshot(ticket.process())?.settled);
    session.session.control().close();
    drop((session, fixture));
    Ok(())
}

#[test]
fn preparation_rejects_a_diagnostic_channel_as_its_attempt_channel() -> TestResult {
    let mut h = Harness::new()?;
    let scope = super::diagnostic::pipe_scope(&mut h)?;
    let diagnostic =
        PurposeKey::Diagnostic(sugarsubstitute_contracts::incident::DiagnosticKey::new(
            owner()?,
            sugarsubstitute_contracts::incident::DiagnosticRole::Storage,
            sugarsubstitute_contracts::incident::DiagnosticSessionId::new(
                (u64::from(std::process::id()) << 32) | 0xffff_0001,
            )?,
        ));
    let (session, bootstrap, enrollment) =
        super::diagnostic::purpose_pins(&mut h, diagnostic, &scope)?;
    let pins = crate::UpdateAttemptPins { bootstrap, enrollment };
    assert!(
        preparation_with(
            &mut h,
            Some(sugarsubstitute_contracts::application_launch::updater::CURRENT_UPDATER_CONTRACT),
            Some(pins),
        )
        .is_err()
    );
    session.session.control().close();
    Ok(())
}

#[test]
fn missing_image_and_wrong_role_are_rejected_before_native_creation() -> TestResult {
    let mut h = Harness::new()?;
    assert!(matches!(
        submit(&mut h, FamilyBinding::Preparation(owner()?), input()?),
        Err(RegistryAdmissionError::InvalidSpecification)
    ));
    let (fixture, input, executable) = preparation(&mut h)?;
    let mut foreign = owner()?;
    foreign.epoch = CoordinatorEpoch::new([2; 16])?;
    assert!(matches!(
        h.registry.validate_input(FamilyBinding::Preparation(foreign), &input),
        Err(RegistryAdmissionError::InvalidSpecification)
    ));
    assert!(!writable(&executable));
    assert!(matches!(
        submit(&mut h, binding(1)?, input),
        Err(RegistryAdmissionError::InvalidSpecification)
    ));
    assert!(writable(&executable));
    assert_eq!(h.fake.creates.load(Ordering::Acquire), 0);
    assert!(h.registry.is_empty());
    fixture.preserved().map_err(|cause| cause as Box<dyn Error>)?;
    Ok(())
}

#[test]
fn retained_preparation_excludes_duplicate_and_registry_loss_closes_only_owned_family() -> TestResult
{
    let mut h = Harness::new()?;
    let (fixture, input, executable) = preparation(&mut h)?;
    let ticket = submit(&mut h, FamilyBinding::Preparation(owner()?), input)?;
    h.created(ticket.process())?;
    let (other, second, second_executable) = preparation(&mut h)?;
    let watch = h.spec(JobClass::NativeObservation, 4096)?;
    let create = h.spec(JobClass::InfrastructureLifecycle, second.required_reservation_bytes())?;
    let native = ScopeSpec { id: ScopeId::new(h.next_scope)?, deadline: None };
    h.next_scope += 1;
    let mut rejected = Some(second);
    assert!(matches!(
        h.registry.submit_preparation_launch(
            &h.client,
            &h.scope,
            native,
            create,
            watch,
            &mut rejected
        ),
        Err(RegistryAdmissionError::Busy)
    ));
    assert_eq!(h.fake.creates.load(Ordering::Acquire), 1);
    assert!(rejected.is_some());
    assert!(!writable(&second_executable));
    storage(&mut h, 4096, move |_| {
        drop(rejected);
        Ok(())
    })?;
    assert!(writable(&second_executable));
    assert!(!writable(&executable));
    let replacement = h.construct_registry(owner()?)?;
    drop(std::mem::replace(&mut h.registry, replacement));
    h.fake.closed()?;
    assert!(writable(&executable));
    fixture.preserved().map_err(|cause| cause as Box<dyn Error>)?;
    other.preserved().map_err(|cause| cause as Box<dyn Error>)?;
    Ok(())
}
