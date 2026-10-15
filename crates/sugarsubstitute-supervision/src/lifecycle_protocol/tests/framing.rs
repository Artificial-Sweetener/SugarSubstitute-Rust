// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject framing/identity ambiguity before payload conversion or output mutation.
use super::*;

#[test]
fn exact_header_layout_and_large_integer_ticks_match_independent_goldens() -> TestResult {
    let encoded = encoded(WirePayload::Register)?;
    let mut golden = Vec::new();
    golden.extend(b"SSLC");
    golden.extend(5u16.to_le_bytes());
    golden.extend(104u16.to_le_bytes());
    golden.extend(104u32.to_le_bytes());
    golden.extend(3u16.to_le_bytes());
    golden.extend(0u16.to_le_bytes());
    golden.extend([0x41; 32]);
    golden.extend([0x42; 16]);
    for value in [7u64, 9, 11, 13, 9_007_199_254_740_993] {
        golden.extend(value.to_le_bytes());
    }
    assert_eq!(encoded, golden);
    assert_eq!(FrameCodec::decode(&golden)?.header, header(3)?);
    assert!(matches!(FrameCodec::decode(&golden)?.payload, DecodedPayload::Register));
    Ok(())
}
#[test]
fn unknown_schema_kind_reserved_and_frame_size_fail_closed() -> TestResult {
    let original = encoded(WirePayload::Register)?;
    for (offset, replacement) in [(0, 0), (4, 1), (6, 103), (12, 0), (12, 19), (14, 1)] {
        let mut frame = original.clone();
        frame[offset] = replacement;
        assert!(FrameCodec::decode(&frame).is_err(), "Offset{offset}");
    }
    let mut frame = original;
    frame[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(FrameCodec::decode(&frame), Err(ProtocolError::FrameLength)));
    assert!(matches!(
        FrameCodec::decode(&vec![0; MAX_FRAME_BYTES + 1]),
        Err(ProtocolError::FrameTooLarge)
    ));
    Ok(())
}
#[test]
fn every_truncated_prefix_and_extra_payload_byte_is_rejected() -> TestResult {
    let source = encoded(WirePayload::Hello(CapabilityRef::new(&[0x73; 32])))?;
    for length in 0..source.len() {
        let mut prefix = source[..length].to_vec();
        if length >= HEADER_BYTES {
            set_length(&mut prefix);
        }
        assert!(matches!(FrameCodec::decode(&prefix), Err(ProtocolError::Truncated)));
    }
    let mut trailing = source;
    trailing.push(0);
    set_length(&mut trailing);
    assert!(matches!(FrameCodec::decode(&trailing), Err(ProtocolError::TrailingBytes)));
    Ok(())
}
#[test]
fn missing_kind_specific_ids_and_zero_epoch_or_generation_are_rejected() -> TestResult {
    for payload in [
        WirePayload::Register,
        WirePayload::Welcome { qpc_frequency: 10_000_000 },
        WirePayload::Hello(CapabilityRef::new(&[0; 32])),
    ] {
        let source = encoded(payload)?;
        for offset in [48usize, 64, 72] {
            let mut invalid = source.clone();
            invalid[offset..offset + if offset == 48 { 16 } else { 8 }].fill(0);
            assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::Identity)));
        }
        let kind = u16::from_le_bytes([source[12], source[13]]);
        let mut invalid = source.clone();
        if kind == 1 {
            invalid[80] = 1;
        } else {
            invalid[80..88].fill(0);
        }
        assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::Identity)));
        let mut invalid = source;
        if kind == 3 {
            invalid[88..96].fill(0);
        } else {
            invalid[88] = 1;
        }
        assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::Identity)));
    }
    Ok(())
}
#[test]
fn positive_qpc_is_required_without_float_rounding_or_authenticated_clock_claims() -> TestResult {
    let mut source = encoded(WirePayload::Welcome { qpc_frequency: i64::MAX })?;
    assert!(matches!(
        FrameCodec::decode(&source)?.payload,
        DecodedPayload::Welcome { qpc_frequency: i64::MAX }
    ));
    for ticks in [0i64, -1, i64::MIN] {
        source[104..112].copy_from_slice(&i64::MAX.to_le_bytes());
        source[96..104].copy_from_slice(&ticks.to_le_bytes());
        assert!(matches!(FrameCodec::decode(&source), Err(ProtocolError::Clock)));
        source[96..104].copy_from_slice(&1i64.to_le_bytes());
        source[104..112].copy_from_slice(&ticks.to_le_bytes());
        assert!(matches!(FrameCodec::decode(&source), Err(ProtocolError::Clock)));
    }
    Ok(())
}
#[test]
fn changed_nonzero_generation_and_request_are_preserved_as_untrusted_claims() -> TestResult {
    let mut source = encoded(WirePayload::Register)?;
    source[72..80].copy_from_slice(&u64::MAX.to_le_bytes());
    source[88..96].copy_from_slice(&u64::MAX.to_le_bytes());
    let decoded = FrameCodec::decode(&source)?;
    assert_eq!(decoded.header.run.generation.get(), u64::MAX);
    assert_eq!(decoded.header.request.map(RequestId::get), Some(u64::MAX));
    assert_ne!(decoded.header.run, header(3)?.run);
    Ok(())
}
#[test]
fn failed_encode_preflight_leaves_caller_storage_unchanged() -> TestResult {
    let mut output = [0x72; 120];
    let message = WireMessage {
        header: header(1)?,
        payload: WirePayload::Hello(CapabilityRef::new(&[0; 32])),
    };
    assert!(matches!(
        FrameCodec::encode(&message, &mut output),
        Err(ProtocolError::OutputTooSmall { required: 136, available: 120 })
    ));
    assert_eq!(output, [0x72; 120]);
    let message =
        WireMessage { header: header(2)?, payload: WirePayload::Welcome { qpc_frequency: 0 } };
    assert!(matches!(FrameCodec::encode(&message, &mut output), Err(ProtocolError::Clock)));
    assert_eq!(output, [0x72; 120]);
    Ok(())
}

#[test]
fn schema5_nonfinal_acquire_contract_rejects_earlier_and_future_peers() -> TestResult {
    let original = encoded(WirePayload::Reply(ReplyOutcome::Pending(lease()?)))?;
    assert_eq!(u16::from_le_bytes([original[4], original[5]]), 5);
    assert!(matches!(
        FrameCodec::decode(&original)?.payload,
        DecodedPayload::Reply(ReplyOutcome::Pending(_))
    ));
    for version in [1u16, 2, 3, 4, 6, u16::MAX] {
        let mut frame = original.clone();
        frame[4..6].copy_from_slice(&version.to_le_bytes());
        assert!(FrameCodec::decode(&frame).is_err());
    }
    Ok(())
}
