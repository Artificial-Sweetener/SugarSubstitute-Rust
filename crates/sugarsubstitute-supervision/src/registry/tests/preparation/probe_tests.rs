// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Signed inert payloads exercise real custody and journals with process effects at the fake boundary.
use super::*;
use crate::setup_storage::{TrustStore, UpdaterPackage, test_package::updater, test_worker};
use sugarsubstitute_contracts::{
    application_launch::updater::CURRENT_UPDATER_CONTRACT,
    installation::{Candidate, InstallationRecord},
    update::Component,
};

mod continuation;
mod withdrawal;

struct ProbeFixture {
    root: tempfile::TempDir,
    _workspace: tempfile::TempDir,
    executable: std::path::PathBuf,
}
fn probe(h: &mut Harness) -> Result<(ProbeFixture, WindowsLaunchInput), Box<dyn Error>> {
    let owner = owner()?;
    storage(
        h,
        SIGNED_INSTALLER_RESERVATION_BYTES + setup_storage::UPDATER_STAGING_RESERVATION_BYTES,
        move |context| {
            let root = test_worker::installation(170, &context)?;
            let workspace = tempfile::tempdir()?;
            let (bytes, _) = updater::archive(CURRENT_UPDATER_CONTRACT)?;
            let (trust, offer, download) = updater::evidence_version(
                root.path(),
                &bytes,
                Component::Updater,
                sugarsubstitute_update::ENGINE_RELEASE.try_into()?,
                &context,
            )?;
            let transfer = UpdaterPackage::import(
                root.path(),
                trust,
                offer,
                download,
                workspace.path(),
                || NOW,
                &context,
            )?
            .stage(|| NOW, &context)?
            .reserve_trial(&context)?
            .into_probe(|| NOW, &context)?;
            let executable = transfer.executable();
            let input = WindowsLaunchInput::updater_probe(
                owner,
                &mut Some(transfer),
                crate::LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?,
            )?;
            assert!(input.is_updater_probe());
            Ok((ProbeFixture { root, _workspace: workspace, executable }, input))
        },
    )
}
fn selection(
    h: &mut Harness,
    fixture: ProbeFixture,
) -> Result<(ProbeFixture, InstallationRecord), Box<dyn Error>> {
    storage(
        h,
        setup_storage::TRUST_RESERVATION_BYTES + setup_storage::TRUST_OPERATION_RESERVATION_BYTES,
        move |context| {
            let trust = TrustStore::open(fixture.root.path(), [92; 32], &context)?;
            let state = trust.updater_selection(&context)?;
            Ok((fixture, state))
        },
    )
}
/// Retire the fake's unbound placeholder at its boundary, then deliver a scoped typed exit.
/// Production completion, identity validation and settlement logic remain unchanged.
fn observed_probe(h: &mut Harness, key: ProcessKey, code: u32, stop_first: bool) -> TestResult {
    let _held = super::super::observation_retirement::created_with_deferred_watch(h, key)?;
    let index = h.registry.index(key)?;
    let placeholder = h.registry.records[index]
        .as_mut()
        .ok_or("Record absent")?
        .root_watch
        .take()
        .ok_or("Placeholder absent")?;
    let placeholder = RefCell::new(placeholder);
    let terminal = RefCell::new(None);
    h.drain_until(|_| {
        *terminal.borrow_mut() = placeholder.borrow_mut().try_take_completion();
        terminal.borrow().is_some()
    })?;
    assert!(matches!(
        terminal.into_inner().ok_or("Placeholder terminal absent")?.outcome,
        JobOutcome::Cancelled
    ));
    assert!(!h.registry.snapshot(key)?.failed);
    if stop_first {
        h.stop(key)?;
    }
    let spec = h.spec(JobClass::InfrastructureLifecycle, 4096)?;
    let exit = ProcessExit {
        correlation: sugarsubstitute_execution::windows::NativeCorrelation::new(key.id.get())?,
        process_id: ROOT.pid,
        creation_time_100ns: ROOT.birth,
        exit_code: code,
    };
    let observed = h.client.submit_blocking(&h.scope, spec, move |_| Ok(exit))?;
    let record = h.registry.records[index].as_mut().ok_or("Record absent")?;
    record.root_spec = (observed.scope_id(), observed.spec().id);
    record.root_watch = Some(observed);
    record.root_notice = false;
    h.drain_until(|registry| {
        registry.snapshot(key).is_ok_and(|snapshot| snapshot.root_exit == Some(code))
    })?;
    h.fake.root_alive.store(false, Ordering::Release);
    h.stop(key)?;
    Ok(())
}
fn transfer(h: &mut Harness, key: ProcessKey) -> Result<SettledUpdaterProbe, Box<dyn Error>> {
    while h.registry.take_observation(key)?.is_some() {}
    assert!(matches!(h.registry.retire_settled(key), Err(RegistryAdmissionError::Busy)));
    let result = h.registry.take_settled_updater_probe(key)?;
    assert!(matches!(
        h.registry.take_settled_updater_probe(key),
        Err(RegistryAdmissionError::Closed)
    ));
    h.registry.retire_settled(key)?;
    Ok(result)
}

#[test]
fn successful_probe_keeps_original_exclusion_and_pins_until_authenticated_publication() -> TestResult
{
    let mut h = Harness::new()?;
    let (fixture, input) = probe(&mut h)?;
    let expected = input
        .preparation
        .as_ref()
        .and_then(|launch| launch.probe())
        .ok_or("Probe absent")?
        .attempt()
        .generation
        .clone();
    let key = submit(&mut h, FamilyBinding::Preparation(owner()?), input)?.process();
    observed_probe(&mut h, key, 0, false)?;
    let inherited = *lock(&h.fake.inherited);
    assert_eq!(inherited.1, 1);
    assert_ne!(inherited.0[0], 0);
    assert_eq!(&inherited.0[1..], &[0; 5]);
    assert!(!inherited.3);
    assert!(matches!(
        h.registry.take_settled_updater_probe(key),
        Err(RegistryAdmissionError::Busy)
    ));
    h.cleanup(key)?;
    assert!(h.registry.snapshot(key)?.settled);
    assert!(!writable(&fixture.executable));
    let mut result = transfer(&mut h, key)?;
    let fixture =
        storage(&mut h, setup_storage::TRUST_OPERATION_RESERVATION_BYTES, move |context| {
            excluded(&fixture)?;
            result.publish(|| NOW, &context)?;
            result.publish(|| NOW, &context)?;
            excluded(&fixture)?;
            assert!(!writable(&fixture.executable));
            drop(result);
            Ok(fixture)
        })?;
    assert!(writable(&fixture.executable));
    let (_fixture, after) = selection(&mut h, fixture)?;
    assert_eq!(after.accepted(), Some(&expected));
    assert!(after.candidate().is_none());
    assert!(after.rejected().is_empty());
    Ok(())
}

fn rejection(code: u32, stop_first: bool, partial_close: bool) -> TestResult {
    let mut h = Harness::new()?;
    let (fixture, input) = probe(&mut h)?;
    let key = submit(&mut h, FamilyBinding::Preparation(owner()?), input)?.process();
    observed_probe(&mut h, key, code, stop_first)?;
    if partial_close {
        h.fake.fail_close_after_job.store(true, Ordering::Release);
    }
    h.cleanup(key)?;
    if partial_close {
        assert!(!h.registry.snapshot(key)?.settled);
        assert!(matches!(
            h.registry.take_settled_updater_probe(key),
            Err(RegistryAdmissionError::Busy)
        ));
        assert!(!writable(&fixture.executable));
        h.cleanup(key)?;
    }
    let mut result = transfer(&mut h, key)?;
    let fixture =
        storage(&mut h, setup_storage::TRUST_OPERATION_RESERVATION_BYTES, move |context| {
            result.publish(|| sugarsubstitute_update::fixture::EXPIRES, &context)?;
            drop(result);
            Ok(fixture)
        })?;
    let (_fixture, state) = selection(&mut h, fixture)?;
    assert!(state.accepted().is_none());
    assert!(state.candidate().is_none());
    assert_eq!(state.rejected().len(), 1);
    Ok(())
}
#[test]
fn nonzero_probe_exit_is_quarantined_after_settlement() -> TestResult {
    rejection(17, false, false)
}
#[test]
fn stopping_before_a_zero_exit_cannot_authorize_activation() -> TestResult {
    rejection(0, true, false)
}
#[test]
fn partial_native_close_retains_custody_and_prevents_acceptance() -> TestResult {
    rejection(0, false, true)
}

/// Native sharing failure proves the original exclusion remains held, without opening another trust store.
fn excluded(fixture: &ProbeFixture) -> Result<(), Failure> {
    let result =
        std::fs::File::open(fixture.root.path().join(".update-trust/updater-preparation-v1"));
    assert!(matches!(result, Err(cause) if cause.raw_os_error() == Some(32)));
    Ok(())
}
fn journal(fixture: &ProbeFixture) -> std::io::Result<Vec<u8>> {
    std::fs::read(fixture.root.path().join(".update-trust/updater-selection-v1"))
}
/// Open a new store only after original trial custody and its reservation have actually retired.
fn recovered(h: &mut Harness, fixture: ProbeFixture) -> Result<ProbeFixture, Box<dyn Error>> {
    storage(
        h,
        setup_storage::TRUST_RESERVATION_BYTES + setup_storage::TRUST_OPERATION_RESERVATION_BYTES,
        move |context| {
            let trust = TrustStore::open(fixture.root.path(), [92; 32], &context)?;
            let before = trust.updater_selection(&context)?;
            assert!(matches!(before.candidate(), Some(Candidate::Trial { .. })));
            assert!(before.accepted().is_none());
            let after = trust.recover_updater_trial(&context)?;
            assert!(after.candidate().is_none());
            assert!(after.accepted().is_none());
            assert_eq!(after.rejected().len(), 1);
            Ok(fixture)
        },
    )
}
#[test]
fn journal_failure_keeps_settled_custody_for_exact_retry() -> TestResult {
    let mut h = Harness::new()?;
    let (fixture, input) = probe(&mut h)?;
    let key = submit(&mut h, FamilyBinding::Preparation(owner()?), input)?.process();
    observed_probe(&mut h, key, 0, false)?;
    h.cleanup(key)?;
    let mut result = transfer(&mut h, key)?;
    let fixture =
        storage(&mut h, setup_storage::TRUST_OPERATION_RESERVATION_BYTES, move |context| {
            let before = journal(&fixture)?;
            let pin = updater::pin_journal(fixture.root.path())?;
            assert!(result.publish(|| NOW, &context).is_err());
            assert_eq!(journal(&fixture)?, before);
            excluded(&fixture)?;
            drop(pin);
            result.publish(|| NOW, &context)?;
            excluded(&fixture)?;
            drop(result);
            Ok(fixture)
        })?;
    assert!(writable(&fixture.executable));
    let (_fixture, state) = selection(&mut h, fixture)?;
    assert!(state.accepted().is_some());
    assert!(state.candidate().is_none());
    Ok(())
}
#[test]
fn expired_acceptance_keeps_trial_without_selecting_or_unlocking_candidate() -> TestResult {
    let mut h = Harness::new()?;
    let (fixture, input) = probe(&mut h)?;
    let key = submit(&mut h, FamilyBinding::Preparation(owner()?), input)?.process();
    observed_probe(&mut h, key, 0, false)?;
    h.cleanup(key)?;
    let mut result = transfer(&mut h, key)?;
    let fixture =
        storage(&mut h, setup_storage::TRUST_OPERATION_RESERVATION_BYTES, move |context| {
            let before = journal(&fixture)?;
            assert!(result.publish(|| sugarsubstitute_update::fixture::EXPIRES, &context).is_err());
            assert_eq!(journal(&fixture)?, before);
            excluded(&fixture)?;
            drop(result);
            Ok(fixture)
        })?;
    let fixture = recovered(&mut h, fixture)?;
    assert!(writable(&fixture.executable));
    Ok(())
}

#[test]
fn failed_creation_with_no_process_restores_preparation_without_quarantine() -> TestResult {
    let mut h = Harness::new()?;
    let (fixture, input) = probe(&mut h)?;
    h.fake.fail_before_create.store(true, Ordering::Release);
    let key = submit(&mut h, FamilyBinding::Preparation(owner()?), input)?.process();
    h.created(key)?;
    h.stop(key)?;
    h.cleanup(key)?;
    assert!(h.registry.snapshot(key)?.failed);
    assert_eq!(h.fake.creates.load(Ordering::Acquire), 0);
    let mut result = transfer(&mut h, key)?;
    let fixture =
        storage(&mut h, setup_storage::TRUST_OPERATION_RESERVATION_BYTES, move |context| {
            result.publish(|| sugarsubstitute_update::fixture::EXPIRES, &context)?;
            drop(result);
            Ok(fixture)
        })?;
    let (_fixture, state) = selection(&mut h, fixture)?;
    assert!(matches!(state.candidate(), Some(Candidate::Prepared { .. })));
    assert!(state.accepted().is_none());
    assert!(state.rejected().is_empty());
    Ok(())
}

#[test]
fn settled_result_drop_preserves_trial_for_exclusive_crash_recovery() -> TestResult {
    let mut h = Harness::new()?;
    let (fixture, input) = probe(&mut h)?;
    let key = submit(&mut h, FamilyBinding::Preparation(owner()?), input)?.process();
    observed_probe(&mut h, key, 0, false)?;
    h.cleanup(key)?;
    let result = transfer(&mut h, key)?;
    let fixture = storage(&mut h, 4096, move |_| {
        excluded(&fixture)?;
        drop(result);
        Ok(fixture)
    })?;
    let fixture = recovered(&mut h, fixture)?;
    assert!(writable(&fixture.executable));
    Ok(())
}
