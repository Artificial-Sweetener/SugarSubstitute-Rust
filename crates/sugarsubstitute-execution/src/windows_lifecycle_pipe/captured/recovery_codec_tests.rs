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

#[test]
fn recovery_narrowed_enrollment_roundtrip_preserves_root_authority_and_capture_cutoff() -> TestResult
{
    let (bytes, data, _) = combined()?;
    let authority = bootstrap::encode(&data)?;
    let narrowed = crate::lifecycle_pipe::LifecyclePipeDeadline::new(950, 100)?;
    assert!(
        bootstrap::decode(bytes[..1024].try_into()?, narrowed, bootstrap::Kind::Desktop).is_err()
    );
    let (decoded, facts) = decode(&bytes, narrowed, bootstrap::Kind::Desktop)?;
    assert_eq!(decoded.deadline, narrowed);
    assert_eq!((facts.source.ticks(), facts.source.frequency()), (900, 100));
    assert_eq!(decoded.capability, data.capability);
    assert_eq!(decoded.name, data.name);
    assert_eq!(&bytes[..1024], &authority);
    assert_eq!(
        bootstrap::decode(&authority, data.deadline, bootstrap::Kind::Desktop)?.deadline,
        data.deadline
    );
    Ok(())
}
#[test]
fn recovery_enrollment_accepts_only_the_closed_source_through_root_interval() -> TestResult {
    let (bytes, _, _) = combined()?;
    for ticks in [900, 901, 999, 1000] {
        let supplied = crate::lifecycle_pipe::LifecyclePipeDeadline::new(ticks, 100)?;
        assert_eq!(decode(&bytes, supplied, bootstrap::Kind::Desktop)?.0.deadline, supplied);
    }
    for (ticks, frequency) in [(899, 100), (1001, 100), (950, 99), (950, 101)] {
        let supplied = crate::lifecycle_pipe::LifecyclePipeDeadline::new(ticks, frequency)?;
        assert!(decode(&bytes, supplied, bootstrap::Kind::Desktop).is_err());
    }
    let narrowed = crate::lifecycle_pipe::LifecyclePipeDeadline::new(950, 100)?;
    for (at, value) in
        [(88, 949i64), (88, -1), (96, 0), (96, 101), (1080, 951), (1080, i64::MAX), (1088, 101)]
    {
        let mut changed = bytes.clone();
        changed[at..at + 8].copy_from_slice(&value.to_le_bytes());
        assert!(
            decode(&changed, narrowed, bootstrap::Kind::Desktop).is_err(),
            "counter field {at}"
        );
    }
    Ok(())
}
#[test]
fn recovery_narrowing_keeps_root_run_capability_schema_name_and_padding_strict() -> TestResult {
    let (bytes, _, _) = combined()?;
    let narrowed = crate::lifecycle_pipe::LifecyclePipeDeadline::new(950, 100)?;
    for at in [0, 4, 6, 8, 40, 56, 64, 138, 144, 672, 1028, 1030, 1856] {
        let mut changed = bytes.clone();
        changed[at] ^= 1;
        assert!(
            decode(&changed, narrowed, bootstrap::Kind::Desktop).is_err(),
            "authority field {at}"
        );
    }
    let mut no_capability = bytes.clone();
    no_capability[104..136].fill(0);
    assert!(decode(&no_capability, narrowed, bootstrap::Kind::Desktop).is_err());
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(decode(&trailing, narrowed, bootstrap::Kind::Desktop).is_err());
    Ok(())
}
#[test]
fn recovery_narrowing_does_not_change_ordinary_or_sscg1_deadline_equality() -> TestResult {
    let (data, endpoint) = fixtures()?;
    let narrowed = crate::lifecycle_pipe::LifecyclePipeDeadline::new(950, 100)?;
    let root = bootstrap::encode(&data)?;
    assert!(bootstrap::decode(&root, narrowed, bootstrap::Kind::Desktop).is_err());
    assert!(bootstrap::decode(&root, data.deadline, bootstrap::Kind::Desktop).is_ok());
    let mut ordinary = vec![0; BASE_BYTES + PREFIX_BYTES + 1];
    encode(&mut ordinary, &data, endpoint, [81, 82], CounterDeadline::new(2000, 100)?, &[1])?;
    assert!(decode(&ordinary, narrowed, bootstrap::Kind::Desktop).is_err());
    assert_eq!(
        decode(&ordinary, data.deadline, bootstrap::Kind::Desktop)?.0.deadline,
        data.deadline
    );
    Ok(())
}
