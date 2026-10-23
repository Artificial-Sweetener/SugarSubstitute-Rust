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
use std::{cell::RefCell, path::Path};
use sugarsubstitute_contracts::execution::{JobContext, JobOutcome};
use sugarsubstitute_update::fixture::NOW;

type Failure = Box<dyn Error + Send + Sync>;
mod image_tests;
mod probe_tests;

/// Admit native storage phases against the same ledger that retains the registry and families.
fn storage<T: Send + 'static>(
    h: &mut Harness,
    bytes: u64,
    body: impl FnOnce(JobContext) -> Result<T, Failure> + Send + 'static,
) -> Result<T, Box<dyn Error>> {
    let mut spec = h.spec(JobClass::InfrastructureLifecycle, bytes)?;
    spec.deadline = Some(Moment::from_nanos(2_000_000_000));
    let job = RefCell::new(h.client.submit_blocking(&h.scope, spec, body)?);
    let completion = RefCell::new(None);
    h.drain_until(|_| {
        *completion.borrow_mut() = job.borrow_mut().try_take_completion();
        completion.borrow().is_some()
    })?;
    match completion.into_inner().ok_or("Storage completion absent")?.outcome {
        JobOutcome::Success(value) => Ok(value),
        JobOutcome::Failure(cause) => {
            let mut next: Option<&(dyn Error + 'static)> = Some(cause.as_ref());
            for _ in 0..16 {
                let Some(error) = next else { break };
                eprintln!("Native storage fixture: {error}");
                next = error.source();
            }
            Err(cause)
        }
        other => Err(format!("Storage job stopped: {:?}", other.kind()).into()),
    }
}

/// Create a real accepted inert package, then retain only independently verified helper files.
fn preparation(
    h: &mut Harness,
) -> Result<(Fixture, WindowsLaunchInput, std::path::PathBuf), Box<dyn Error>> {
    preparation_with(
        h,
        Some(sugarsubstitute_contracts::application_launch::updater::CURRENT_UPDATER_CONTRACT),
    )
}

fn preparation_with(
    h: &mut Harness,
    updater: Option<&'static [u8]>,
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
        )?;
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
