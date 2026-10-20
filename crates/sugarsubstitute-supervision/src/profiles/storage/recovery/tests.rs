// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise abandoned and surviving witnesses using real protected file locks.
use super::*;
use crate::coordinator_entry::fixture::Harness;
use sugarsubstitute_contracts::{
    backend_custody::{BackendCustodyKey, BackendCustodySession, ProfileCustodyKey},
    execution::{ExecutionPort, JobOutcome},
};

#[test]
fn abandoned_profile_recovers_without_deleting_browser_data_and_live_witness_blocks()
-> Result<(), Box<dyn std::error::Error>> {
    let mut harness = Harness::new()?;
    let until = Moment::from_nanos(15_000_000_000);
    let mut spec = harness.spec(Moment::from_nanos(30_000_000_000))?;
    spec.reservation_bytes = crate::profiles::PROFILE_OPERATION_BYTES;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let base = std::env::temp_dir().join(format!(
        "sugarsubstitute-abandoned-profile-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let mut job = harness.client.submit_blocking(&harness.scope, spec, move |context| {
        let base = crate::protected_storage::profile_fixture::create(&base, &context)?;
        let profile = base.join("profile");
        let run = super::super::tests::generation(1)?.run;
        let port = WindowsProfilePort;
        let mut prior = NativeState::new();
        port.acquire(&mut prior, &profile, run, &context)?;
        let row = record::custody::CustodiedRecord::new(
            prior.record.ok_or_else(|| ClaimError::invalid("test_profile_record"))?,
            BackendCustodyKey::new(
                run.owner,
                BackendCustodySession::new(1)
                    .map_err(|cause| ClaimError::new("test_session", cause))?,
            ),
            ProfileCustodyKey::new(run, 1)
                .map_err(|cause| ClaimError::new("test_profile", cause))?,
        )?;
        let writer = prior.file.as_ref().ok_or_else(|| ClaimError::invalid("test_writer"))?;
        record_file::publish(
            writer,
            &row.encode(),
            record_file::PublicationAuthority::Ordinary,
            &context,
            |_| Ok(()),
        )?;
        let budget = writer.handle.budget();
        let sid = writer.account_sid.bytes()?.to_vec();
        let mut witness = None;
        HeldFile::<160>::acquire_into(
            ExclusionOpen::CreateOrOpen,
            &mut witness,
            NativeName::Ordinary(profile.join(".sugarsubstitute-profile-v2.custody")),
            NativeName::Ordinary(profile.join(".sugarsubstitute-profile-v1.record")),
            NativeSid::Ordinary(sid),
            budget.clone(),
            &mut NativePhase::ordinary(&budget)?,
        )?;
        prior.file.as_mut().ok_or_else(|| ClaimError::invalid("test_writer"))?.close()?;
        prior.file = None;
        prior.root.as_mut().ok_or_else(|| ClaimError::invalid("test_root"))?.retire(&context)?;
        prior.root = None;
        drop(prior);
        let marker = profile.join(".sugarsubstitute-profile-v1.record");
        let mut denied = NativeState::new();
        assert!(port.acquire(&mut denied, &profile, run, &context).is_err());
        port.release(&mut denied, &context)?;
        drop(denied);
        assert_eq!(
            std::fs::read(&marker).map_err(|cause| ClaimError::new("test_read", cause))?,
            row.encode()
        );
        witness.as_mut().ok_or_else(|| ClaimError::invalid("test_witness"))?.close()?;
        drop((witness, budget));
        let browser_data = profile.join("preferences.json");
        std::fs::write(&browser_data, b"user preferences")
            .map_err(|cause| ClaimError::new("test_preferences", cause))?;
        let mut next = NativeState::new();
        let successor = super::super::tests::generation(2)?.run;
        port.acquire(&mut next, &profile, successor, &context)?;
        assert!(next.held && next.recovery.admitted());
        assert_eq!(next.predecessor, Some(row.encode()));
        let backups = std::fs::read_dir(&profile)
            .map_err(|cause| ClaimError::new("test_backups", cause))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|cause| ClaimError::new("test_backups", cause))?;
        let backup = backups
            .iter()
            .find(|entry| {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                name.starts_with(".sugarsubstitute-profile-recovered-") && name.ends_with(".record")
            })
            .ok_or_else(|| ClaimError::invalid("test_missing_backup"))?;
        assert_eq!(
            std::fs::read(backup.path()).map_err(|cause| ClaimError::new("test_backup", cause))?,
            row.encode()
        );
        Ok::<_, ClaimError>((base, profile, browser_data, next, run))
    })?;
    let acquired = harness.consume_job(&mut job)?;
    let JobOutcome::Success((base, profile, browser_data, mut next, run)) = acquired.outcome else {
        return Err("Abandoned profile did not recover under its original startup budget".into());
    };
    drop(job);
    let mut cleanup_spec = harness.spec(until)?;
    cleanup_spec.reservation_bytes = crate::profiles::PROFILE_OPERATION_BYTES;
    let mut cleanup =
        harness.client.submit_blocking(&harness.scope, cleanup_spec, move |context| {
            let port = WindowsProfilePort;
            port.release(&mut next, &context)?;
            assert_eq!(
                std::fs::read(&browser_data)
                    .map_err(|cause| ClaimError::new("test_read", cause))?,
                b"user preferences"
            );
            drop(next);
            let mut reopened = NativeState::new();
            port.acquire(&mut reopened, &profile, run, &context)?;
            port.release(&mut reopened, &context)?;
            drop(reopened);
            std::fs::remove_dir_all(&base)
                .map_err(|cause| ClaimError::new("test_cleanup", cause))?;
            Ok::<_, ClaimError>(())
        })?;
    let done = harness.consume_job(&mut cleanup)?;
    assert!(matches!(done.outcome, JobOutcome::Success(())), "{done:?}");
    drop((done, cleanup));
    drop((acquired.memory, acquired.native_handles));
    harness.close(until)?;
    harness.join_execution_after_retirement()?;
    assert_eq!(harness.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
