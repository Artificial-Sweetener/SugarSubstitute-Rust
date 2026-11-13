// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject purpose/header/payload substitution in the original captured section.
use super::{super::native::bootstrap, codec::*};
use crate::{
    capture_attach::types::*,
    lifecycle_pipe::{LifecyclePipeDeadline, PeerBinding},
    runtime_test_support::TestResult,
};
use sugarsubstitute_contracts::{
    backend_custody::{BackendCustodyKey, BackendCustodySession},
    crash::{CaptureBudget, CaptureTarget},
    execution::{CounterDeadline, Moment},
    incident::BuildIdentity,
    lifecycle::*,
};
fn bytes() -> TestResult<(Vec<u8>, bootstrap::Data, BackendCustodyKey)> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let key = BackendCustodyKey::new(owner, BackendCustodySession::new(9)?);
    let creator = CaptureProcessFacts::new(71, 99)?;
    let handler = CaptureProcessFacts::new(72, 100)?;
    let expectation = CaptureAttachExpectation::new(
        owner,
        CaptureTarget::BackendCustody(key),
        &BuildIdentity::new("build")?,
        creator,
        handler,
        CaptureBudget::new(Moment::from_nanos(0))?,
    )?;
    let endpoint = CaptureEndpointObservation::new(
        &expectation,
        CaptureEndpointProvenance::CreatorInherited,
        creator,
        handler,
        &r"\\.\pipe\backend_capture".encode_utf16().collect::<Vec<_>>(),
    )?;
    let data = bootstrap::Data {
        binding: PeerBinding::BackendCustody(key),
        pid: 71,
        birth: 99,
        session: 1,
        deadline: LifecyclePipeDeadline::new(1000, 100)?,
        capability: [7; 32],
        name: bootstrap::name(PeerBinding::BackendCustody(key)),
    };
    let mut out = vec![0; BASE_BYTES + PREFIX_BYTES];
    encode(&mut out, &data, endpoint, [81, 82], CounterDeadline::new(2000, 100)?, &[])?;
    Ok((out, data, key))
}
#[test]
fn backend_captured_v5_preserves_actual_key_process_facts_and_original_counters() -> TestResult {
    let (out, data, key) = bytes()?;
    assert_eq!(out.len(), 2064);
    assert_eq!(&out[4..8], &[5, 0, 0, 8]);
    assert_eq!(&out[1032..1036], &2u32.to_le_bytes());
    let (decoded, facts) = decode(&out, data.deadline, bootstrap::Kind::BackendCustody)?;
    assert_eq!(decoded.binding, PeerBinding::BackendCustody(key));
    assert_eq!(facts.processes, [81, 82]);
    assert_eq!(
        (facts.creator_pid, facts.creator_birth, facts.handler_pid, facts.handler_birth),
        (71, 99, 72, 100)
    );
    assert_eq!((facts.source.ticks(), facts.source.frequency()), (2000, 100));
    assert!(facts.payload.is_empty());
    assert!(decode(&out, data.deadline, bootstrap::Kind::Desktop).is_err());
    let old = bootstrap::encode(&data)?;
    assert_eq!(&old[4..8], &[4, 0, 0, 4]);
    assert_eq!(
        bootstrap::decode(&old, data.deadline, bootstrap::Kind::BackendCustody)?.binding,
        data.binding
    );
    assert!(decode(&old, data.deadline, bootstrap::Kind::BackendCustody).is_err());
    Ok(())
}
#[test]
fn backend_captured_purpose_reserved_fields_and_payload_fail_closed() -> TestResult {
    let (valid, data, _) = bytes()?;
    for offset in
        [4, 6, 60, 64, 1024, 1028, 1032, 1036, 1040, 1048, 1096, 1112, 1116, 1696, 2052, 2054, 2060]
    {
        let mut out = valid.clone();
        out[offset] ^= 1;
        assert!(
            decode(&out, data.deadline, bootstrap::Kind::BackendCustody).is_err(),
            "accepted offset {offset}"
        );
    }
    for empty in [1044..1048, 1056..1064, 1064..1072, 1072..1080, 1080..1088, 1088..1096] {
        let mut out = valid.clone();
        out[empty].fill(0);
        assert!(decode(&out, data.deadline, bootstrap::Kind::BackendCustody).is_err());
    }
    let mut payload = valid.clone();
    payload[2056..2060].copy_from_slice(&1u32.to_le_bytes());
    payload.push(1);
    assert!(decode(&payload, data.deadline, bootstrap::Kind::BackendCustody).is_err());
    let mut trailing = valid.clone();
    trailing.push(0);
    assert!(decode(&trailing, data.deadline, bootstrap::Kind::BackendCustody).is_err());
    Ok(())
}
#[test]
fn backend_map_preflight_fits_original_64_kib_metadata_and_rejects_oversized_native_owner()
-> TestResult {
    use super::super::{captured_layout::CapturedLayout, native};
    let layout = CapturedLayout::backend(
        4096,
        super::super::startup_mapping::StartupMapping::control_storage_bytes(),
        native::Lease::storage_bytes(),
    )?;
    assert_eq!(layout.map_extent, 4096);
    assert_eq!(layout.map_bytes + layout.native_bytes, 65536);
    assert!(CapturedLayout::backend(4096, 4097, 1).is_err());
    assert!(CapturedLayout::backend(4096, 4096, 65536).is_err());
    assert!(CapturedLayout::backend(0, 1, 1).is_err());
    Ok(())
}
