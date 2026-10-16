// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve direct Root lifecycle authority while independently matching fresh capture.
use super::{codec::*, codec_tests::fixtures};
use crate::{
    lifecycle_pipe::PeerBinding, runtime_test_support::TestResult,
    windows_lifecycle_pipe::native::bootstrap,
};
use sugarsubstitute_contracts::{execution::CounterDeadline, incident::*, recovery_supervisor::*};
fn combined() -> TestResult<(Vec<u8>, bootstrap::Data, RecoveryGrantKey)> {
    let (mut data, endpoint) = fixtures()?;
    let run = data.binding.desktop().ok_or("desktop fixture")?;
    let previous = sugarsubstitute_contracts::lifecycle::RunKey {
        generation: sugarsubstitute_contracts::lifecycle::RunGeneration::new(
            run.generation.get() - 1,
        )?,
        ..run
    };
    let incident = IncidentKey::new(IncidentSubject::Desktop(previous));
    let report = ReportBinding::new(incident, IncidentRevision::new(1)?, ReportSessionId::new(3)?);
    let key = RecoveryGrantKey::new(RecoveryGrantId::new(5)?, previous, report)?;
    data.pid = 61;
    data.birth = 90;
    let root = bootstrap::encode(&data)?;
    let mut bytes = vec![0; BASE_BYTES + PREFIX_BYTES + 3];
    encode_recovery(
        &mut bytes,
        &root,
        key,
        endpoint,
        [81, 82],
        CounterDeadline::new(900, 100)?,
        &[1, 2, 3],
    )?;
    Ok((bytes, data, key))
}
#[test]
fn recovery_combined_keeps_original_root_bytes_fresh_creator_and_existing_extent() -> TestResult {
    let (bytes, data, key) = combined()?;
    assert_eq!(&bytes[..1024], &bootstrap::encode(&data)?);
    let (root, facts) = decode(&bytes, data.deadline, bootstrap::Kind::Desktop)?;
    assert_eq!(root.binding, PeerBinding::Desktop(key.run()));
    assert_eq!((root.pid, root.birth), (61, 90));
    assert_eq!((facts.creator_pid, facts.creator_birth), (71, 99));
    assert_eq!(facts.recovery, Some(key));
    assert_eq!(facts.processes, [81, 82]);
    assert_eq!(&bytes[facts.payload], &[1, 2, 3]);
    assert_eq!(MAX_LOGICAL_BYTES, 67480);
    Ok(())
}
#[test]
fn recovery_combined_rejects_cross_root_run_grant_schema_and_legacy_substitution() -> TestResult {
    let (valid, data, _) = combined()?;
    for offset in
        [4, 6, 8, 40, 56, 64, 1028, 1032, 1036, 1696 + 8, 1696 + 40, 1696 + 56, 1696 + 64, 1856]
    {
        let mut bytes = valid.clone();
        bytes[offset] ^= 1;
        assert!(
            decode(&bytes, data.deadline, bootstrap::Kind::Desktop).is_err(),
            "accepted offset {offset}"
        );
    }
    let mut zero = valid.clone();
    zero[1696..1704].fill(0);
    assert!(decode(&zero, data.deadline, bootstrap::Kind::Desktop).is_err());
    let mut old = valid.clone();
    old[1028..1030].copy_from_slice(&1u16.to_le_bytes());
    assert!(decode(&old, data.deadline, bootstrap::Kind::Desktop).is_err());
    let mut old_base = valid.clone();
    old_base[4..8].copy_from_slice(&[3, 0, 0, 8]);
    assert!(decode(&old_base, data.deadline, bootstrap::Kind::Desktop).is_err());
    assert!(decode(&valid, data.deadline, bootstrap::Kind::BackendCustody).is_err());
    let mut expanded = valid.clone();
    expanded[1080..1088].copy_from_slice(&1001i64.to_le_bytes());
    assert!(decode(&expanded, data.deadline, bootstrap::Kind::Desktop).is_err());
    let mut alias = valid.clone();
    alias[1040..1044].copy_from_slice(&data.pid.to_le_bytes());
    assert!(decode(&alias, data.deadline, bootstrap::Kind::Desktop).is_err());
    Ok(())
}
