// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observed engine health supplies a pinned next process without carrying preparation exclusion.
use super::*;
use std::os::windows::fs::OpenOptionsExt;

const PIN_BYTES: u64 =
    setup_storage::RESERVATION_BYTES + setup_storage::TRUST_OPERATION_RESERVATION_BYTES;

#[test]
fn accepted_probe_continues_with_independent_files_and_releases_preparation_exclusion() -> TestResult
{
    let mut h = Harness::new()?;
    let (fixture, input) = probe(&mut h)?;
    let key = submit(&mut h, FamilyBinding::Preparation(owner()?), input)?.process();
    observed_probe(&mut h, key, 0, false)?;
    h.cleanup(key)?;
    let mut settled = transfer(&mut h, key)?;
    let (fixture, image) = storage(&mut h, PIN_BYTES, move |context| {
        assert!(settled.pin_preparation(&context).is_err());
        settled.publish(|| NOW, &context)?;
        let image = settled.pin_preparation(&context)?;
        assert_eq!(image.executable(), fixture.executable);
        excluded(&fixture)?;
        drop(settled);
        assert!(!writable(&fixture.executable));
        Ok((fixture, image))
    })?;
    let owner = owner()?;
    let fixture = storage(
        &mut h,
        setup_storage::TRUST_RESERVATION_BYTES + setup_storage::TRUST_OPERATION_RESERVATION_BYTES,
        move |context| {
            let generation = image.generation().clone();
            let input = WindowsLaunchInput::preparation(
                owner,
                image,
                fixture._workspace.path(),
                crate::LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?,
                None,
            )?;
            let mut trust = TrustStore::open(fixture.root.path(), [92; 32], &context)?;
            trust.acquire_preparation(&context)?;
            drop(
                std::fs::OpenOptions::new()
                    .read(true)
                    .share_mode(0)
                    .open(fixture.root.path().join("state/update-trust/updater-preparation-v1"))?,
            );
            let state = trust.updater_selection(&context)?;
            assert_eq!(state.accepted(), Some(&generation));
            assert!(state.candidate().is_none());
            assert!(!writable(&fixture.executable));
            drop(input);
            assert!(writable(&fixture.executable));
            Ok(fixture)
        },
    )?;
    assert!(writable(&fixture.executable));
    Ok(())
}

#[test]
fn rejected_or_stopped_probe_cannot_supply_a_preparation_image() -> TestResult {
    for (code, stopped) in [(17, false), (0, true)] {
        let mut h = Harness::new()?;
        let (fixture, input) = probe(&mut h)?;
        let key = submit(&mut h, FamilyBinding::Preparation(owner()?), input)?.process();
        observed_probe(&mut h, key, code, stopped)?;
        h.cleanup(key)?;
        let mut settled = transfer(&mut h, key)?;
        storage(&mut h, PIN_BYTES, move |context| {
            assert!(settled.pin_preparation(&context).is_err());
            settled.publish(|| NOW, &context)?;
            assert!(settled.pin_preparation(&context).is_err());
            excluded(&fixture)?;
            drop(settled);
            assert!(writable(&fixture.executable));
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn failed_image_admission_retains_accepted_probe_for_retry() -> TestResult {
    let mut h = Harness::new()?;
    let (fixture, input) = probe(&mut h)?;
    let key = submit(&mut h, FamilyBinding::Preparation(owner()?), input)?.process();
    observed_probe(&mut h, key, 0, false)?;
    h.cleanup(key)?;
    let mut settled = transfer(&mut h, key)?;
    let (fixture, settled) =
        storage(&mut h, setup_storage::TRUST_OPERATION_RESERVATION_BYTES, move |context| {
            settled.publish(|| NOW, &context)?;
            let journal = journal(&fixture)?;
            assert!(settled.pin_preparation(&context).is_err());
            assert_eq!(super::journal(&fixture)?, journal);
            excluded(&fixture)?;
            assert!(!writable(&fixture.executable));
            Ok((fixture, settled))
        })?;
    storage(&mut h, PIN_BYTES, move |context| {
        let image = settled.pin_preparation(&context)?;
        drop(settled);
        assert!(!writable(&fixture.executable));
        drop(image);
        assert!(writable(&fixture.executable));
        Ok(())
    })?;
    Ok(())
}

#[test]
fn unstarted_probe_cannot_supply_an_image_after_restoring_preparation() -> TestResult {
    let mut h = Harness::new()?;
    let (fixture, input) = probe(&mut h)?;
    h.fake.fail_before_create.store(true, Ordering::Release);
    let key = submit(&mut h, FamilyBinding::Preparation(owner()?), input)?.process();
    h.created(key)?;
    h.stop(key)?;
    h.cleanup(key)?;
    let mut settled = transfer(&mut h, key)?;
    storage(&mut h, PIN_BYTES, move |context| {
        settled.publish(|| NOW, &context)?;
        assert!(settled.pin_preparation(&context).is_err());
        excluded(&fixture)?;
        drop(settled);
        Ok(())
    })?;
    Ok(())
}
