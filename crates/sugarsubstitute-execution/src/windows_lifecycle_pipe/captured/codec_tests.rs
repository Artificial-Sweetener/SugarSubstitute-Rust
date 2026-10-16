// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject malformed matching bytes without substituting them for native authority.
use super::{super::native::bootstrap, codec::*};
use crate::{
    capture_attach::types::*,
    lifecycle_pipe::{LifecyclePipeDeadline, PeerBinding},
    runtime_test_support::TestResult,
};
use sugarsubstitute_contracts::{
    crash::CaptureBudget,
    execution::{CounterDeadline, Moment},
    incident::BuildIdentity,
    lifecycle::*,
};

/// Construct inert matching fixtures; no Registered/native handles are produced.
pub(super) fn fixtures() -> TestResult<(bootstrap::Data, CaptureEndpointObservation)> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let run = RunKey { owner, instance: InstanceId::new(7)?, generation: RunGeneration::new(9)? };
    let creator = CaptureProcessFacts::new(71, 99)?;
    let handler = CaptureProcessFacts::new(72, 100)?;
    let expected = CaptureAttachExpectation::new(
        owner,
        sugarsubstitute_contracts::crash::CaptureTarget::Desktop(run),
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
    Ok((
        bootstrap::Data {
            binding: PeerBinding::Desktop(run),
            pid: 71,
            birth: 99,
            session: 1,
            deadline: LifecyclePipeDeadline::new(1000, 100)?,
            capability: [7; 32],
            name: bootstrap::name(PeerBinding::Desktop(run)),
        },
        endpoint,
    ))
}
/// Encode actual schema owners into a bounded destination for rejection tests.
fn bytes(body: &[u8]) -> TestResult<(Vec<u8>, bootstrap::Data)> {
    let (data, endpoint) = fixtures()?;
    let mut out = vec![0; BASE_BYTES + PREFIX_BYTES + body.len()];
    encode(&mut out, &data, endpoint, [81, 82], CounterDeadline::new(2000, 100)?, body)?;
    Ok((out, data))
}
#[test]
fn captured_roundtrip_preserves_original_source_counter_payload_and_reserved_prefix() -> TestResult
{
    let body = [1, 2, 3, 4];
    let (out, data) = bytes(&body)?;
    let (base, facts) = decode(&out, data.deadline, bootstrap::Kind::Desktop)?;
    assert_eq!(base.binding, data.binding);
    assert_eq!(base.deadline, data.deadline);
    assert_eq!(facts.processes, [81, 82]);
    assert_eq!(
        (facts.creator_pid, facts.handler_pid, facts.creator_birth, facts.handler_birth),
        (71, 72, 99, 100)
    );
    assert_eq!((facts.source.ticks(), facts.source.frequency()), (2000, 100));
    assert_eq!(&facts.build[..facts.build_length], b"test_build");
    assert_eq!(&out[facts.payload], &body);
    assert_eq!(
        &facts.endpoint[..facts.endpoint_length],
        &r"\\.\pipe\capture_test".encode_utf16().collect::<Vec<_>>()
    );
    assert!(out[1696..2048].iter().all(|b| *b == 0));
    assert_eq!(MAX_LOGICAL_BYTES, 67480);
    Ok(())
}
#[test]
fn captured_strict_header_tail_and_each_authority_matching_slot_fail_closed() -> TestResult {
    let (valid, data) = bytes(&[1, 2, 3])?;
    for index in [
        0, 4, 6, 138, 1024, 1028, 1030, 1032, 1036, 1040, 1044, 1048, 1056, 1064, 1072, 1080, 1088,
        1096, 1112, 1116, 1120, 1184, 1696, 2048, 2052, 2054, 2056, 2060,
    ] {
        let mut value = valid.clone();
        match index {
            1044 => value[index..index + 4].fill(0),
            1056 | 1064 | 1072 | 1080 | 1088 => value[index..index + 8].fill(0),
            _ => value[index] ^= 0xff,
        }
        assert!(
            decode(&value, data.deadline, bootstrap::Kind::Desktop).is_err(),
            "Accepted malformed offset {index}"
        );
    }
    let mut trailing = valid.clone();
    trailing.push(0);
    assert!(decode(&trailing, data.deadline, bootstrap::Kind::Desktop).is_err());
    assert!(decode(&valid[..valid.len() - 1], data.deadline, bootstrap::Kind::Desktop).is_err());
    assert!(
        bootstrap::decode(valid[..1024].try_into()?, data.deadline, bootstrap::Kind::Desktop)
            .is_err()
    );
    Ok(())
}
#[test]
fn captured_bound_and_ordinary_versions_preserve_exact_extent_compatibility() -> TestResult {
    let (data, endpoint) = fixtures()?;
    let source = CounterDeadline::new(2000, 100)?;
    for length in [0, MAX_PAYLOAD_BYTES + 1] {
        let mut out = vec![0; BASE_BYTES + PREFIX_BYTES + length];
        assert!(encode(&mut out, &data, endpoint, [81, 82], source, &vec![1; length]).is_err());
    }
    let (max, data) = bytes(&vec![1; MAX_PAYLOAD_BYTES])?;
    assert_eq!(max.len(), 67480);
    assert!(decode(&max, data.deadline, bootstrap::Kind::Desktop).is_ok());
    let old = bootstrap::encode(&data)?;
    assert_eq!(&old[4..8], &[1, 0, 0, 4]);
    assert!(bootstrap::decode(&old, data.deadline, bootstrap::Kind::Desktop).is_ok());
    assert!(decode(&old, data.deadline, bootstrap::Kind::Desktop).is_err());
    Ok(())
}
