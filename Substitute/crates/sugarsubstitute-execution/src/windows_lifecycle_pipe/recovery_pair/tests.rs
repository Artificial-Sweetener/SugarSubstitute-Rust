// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native paired-section owner grammar rejects identity substitution before creating a Root loan.
use super::*;
use sugarsubstitute_contracts::{incident::*, lifecycle::*, recovery_supervisor::*};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn recovery() -> Result<bootstrap::Data> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let previous =
        RunKey { owner, instance: InstanceId::new(3)?, generation: RunGeneration::new(4)? };
    let key = RecoveryGrantKey::new(
        RecoveryGrantId::new(5)?,
        previous,
        ReportBinding::new(
            IncidentKey::new(IncidentSubject::Desktop(previous)),
            IncidentRevision::new(1)?,
            ReportSessionId::new(2)?,
        ),
    )?;
    let binding = PeerBinding::Recovery(key);
    Ok(bootstrap::Data {
        binding,
        pid: 7,
        birth: 8,
        session: 9,
        deadline: LifecyclePipeDeadline::new(100, 1000)?,
        capability: [10; 32],
        name: bootstrap::name(binding),
    })
}
fn desktop(source: &bootstrap::Data) -> Result<bootstrap::Data> {
    let binding =
        PeerBinding::Desktop(source.binding.recovery().ok_or("Recovery key absent")?.run());
    Ok(bootstrap::Data {
        binding,
        pid: source.pid,
        birth: source.birth,
        session: source.session,
        deadline: source.deadline,
        capability: [11; 32],
        name: bootstrap::name(binding),
    })
}
#[test]
fn actual_root_desktop_pair_preserves_original_bytes_and_narrows_both_genuine_counters() -> Result {
    let source = recovery()?;
    let mut ui = desktop(&source)?;
    for (ticks, expected) in [(99, 99), (100, 100), (101, 100)] {
        ui.deadline = LifecyclePipeDeadline::new(ticks, 1000)?;
        let bytes = bootstrap::encode(&ui)?;
        let (actual, until) = validate_pair_data(&bytes, &source)?;
        assert_eq!(until.ticks(), expected);
        assert_eq!(until.frequency(), 1000);
        assert_eq!(actual.binding, ui.binding);
        assert_eq!(actual.capability, ui.capability);
        assert_eq!(bootstrap::encode(&actual)?, bytes);
    }
    Ok(())
}
#[test]
fn cross_root_run_pid_birth_session_secret_and_frequency_pairs_fail_before_source_publication()
-> Result {
    let source = recovery()?;
    let ui = desktop(&source)?;
    let bytes = bootstrap::encode(&ui)?;
    for at in [8usize, 40, 56, 64, 72, 76, 80, 96, 138, 672] {
        let mut bad = bytes;
        bad[at] ^= 1;
        assert!(validate_pair_data(&bad, &source).is_err(), "field {at}");
    }
    for capability in [[0; 32], source.capability] {
        let mut bad = bytes;
        bad[104..136].copy_from_slice(&capability);
        assert!(validate_pair_data(&bad, &source).is_err());
    }
    for kind in [3u16, 4, 5, 6, 7] {
        let mut bad = bytes;
        bad[4..6].copy_from_slice(&kind.to_le_bytes());
        assert!(validate_pair_data(&bad, &source).is_err());
    }
    Ok(())
}
#[test]
fn duplicate_zero_overflow_pseudo_and_wrong_direction_section_slots_are_rejected_before_adoption()
-> Result {
    use crate::windows_recovery_supervisor::inheritance::validate_pair_slots;
    assert!(validate_pair_slots(17, 18).is_ok());
    for (first, second) in
        [(0, 18), (17, 0), (17, 17), (u64::MAX, 18), (17, u64::MAX), (usize::MAX as u64 - 15, 18)]
    {
        assert!(validate_pair_slots(first, second).is_err());
    }
    let source = recovery()?;
    let bytes = bootstrap::encode(&source)?;
    assert!(validate_pair_data(&bytes, &source).is_err());
    let ui = desktop(&source)?;
    assert!(validate_pair_data(&bootstrap::encode(&ui)?, &ui).is_err());
    Ok(())
}

#[test]
fn paired_native_owner_minimum_roundtrips_combined_child_without_rewriting_root_authority() -> Result
{
    use crate::capture_attach::types::*;
    use crate::windows_lifecycle_pipe::captured::codec;
    use sugarsubstitute_contracts::{crash::*, execution::Moment};
    let recovery = recovery()?;
    let mut root = desktop(&recovery)?;
    root.deadline = LifecyclePipeDeadline::new(101, 1000)?;
    let authority = bootstrap::encode(&root)?;
    let (_, admitted) = validate_pair_data(&authority, &recovery)?;
    assert_eq!(admitted.ticks(), 100);
    let creator = CaptureProcessFacts::new(71, 99)?;
    let handler = CaptureProcessFacts::new(72, 100)?;
    let key = recovery.binding.recovery().ok_or("recovery fixture")?;
    let expected = CaptureAttachExpectation::new(
        key.run().owner,
        CaptureTarget::Desktop(key.run()),
        &BuildIdentity::new("test_build")?,
        creator,
        handler,
        CaptureBudget::new(Moment::from_nanos(0))?,
    )?;
    let endpoint = CaptureEndpointObservation::new(
        &expected,
        CaptureEndpointProvenance::CreatorInherited,
        creator,
        handler,
        &r"\\.\pipe\capture_test".encode_utf16().collect::<Vec<_>>(),
    )?;
    let mut bytes = vec![0; codec::BASE_BYTES + codec::PREFIX_BYTES + 1];
    codec::encode_recovery(
        &mut bytes,
        &authority,
        key,
        endpoint,
        [81, 82],
        CounterDeadline::new(90, 1000)?,
        &[1],
    )?;
    let supplied = LifecyclePipeDeadline::new(admitted.ticks(), admitted.frequency())?;
    let (actual, facts) = codec::decode(&bytes, supplied, bootstrap::Kind::Desktop)?;
    assert_eq!(actual.deadline, supplied);
    assert_eq!(facts.source.ticks(), 90);
    assert_eq!(&bytes[..1024], &authority);
    assert_eq!(actual.binding, root.binding);
    assert_eq!(actual.capability, root.capability);
    assert!(
        codec::decode(&bytes, LifecyclePipeDeadline::new(102, 1000)?, bootstrap::Kind::Desktop)
            .is_err()
    );
    Ok(())
}
