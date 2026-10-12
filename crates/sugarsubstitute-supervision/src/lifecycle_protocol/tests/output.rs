// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check raw output wire fidelity and bounded delivery shape independently of native custody.
use super::extensions::assert_rejected_tail_and_prefix;
use super::*;
use std::num::NonZeroU64;
use sugarsubstitute_contracts::transcript::{
    MAX_BATCH_BYTES, OutputGap, OutputStream, StreamSequence,
};
fn delivery(bytes: &[u8]) -> Result<OutputDelivery<'_>, Box<dyn Error>> {
    Ok(OutputDelivery {
        lease: lease()?,
        delivery_id: NonZeroU64::new(43).ok_or("zero delivery")?,
        stream: OutputStream::Stderr,
        sequence: StreamSequence::new(47)?,
        gap: Some(OutputGap::new(9, 2)?),
        end: true,
        bytes,
    })
}
#[test]
fn raw_output_maximum_stream_loss_and_eof_remain_exact_borrowed_facts() -> TestResult {
    let bytes = vec![0xFF; MAX_BATCH_BYTES];
    for stream in [OutputStream::Stdout, OutputStream::Stderr] {
        for gap in [None, Some(OutputGap::new(9, 2)?)] {
            for end in [false, true] {
                let value = OutputDelivery { stream, gap, end, ..delivery(&bytes)? };
                let source = encoded(WirePayload::Reply(ReplyOutcome::OutputDelivery(value)))?;
                assert!(source.len() < MAX_FRAME_BYTES);
                assert_eq!(source[104], 12);
                let DecodedPayload::Reply(ReplyOutcome::OutputDelivery(actual)) =
                    FrameCodec::decode(&source)?.payload
                else {
                    return Err("missing output".into());
                };
                assert_eq!(actual, value);
                assert_eq!(actual.bytes.as_ptr(), source[source.len() - bytes.len()..].as_ptr());
                let debug = format!("{actual:?}");
                assert!(!debug.contains("255, 255"));
            }
        }
    }
    for gap in [None, Some(OutputGap::new(1, 0)?)] {
        let value = OutputDelivery { gap, ..delivery(&[])? };
        let source = encoded(WirePayload::Reply(ReplyOutcome::OutputDelivery(value)))?;
        assert!(
            matches!(FrameCodec::decode(&source)?.payload,DecodedPayload::Reply(ReplyOutcome::OutputDelivery(actual)) if actual==value)
        );
    }
    Ok(())
}

#[test]
fn malformed_output_flags_counts_and_positive_fields_fail_closed() -> TestResult {
    let source = encoded(WirePayload::Reply(ReplyOutcome::OutputDelivery(delivery(&[0xFF, 0])?)))?;
    assert_rejected_tail_and_prefix(&source)?;
    for (offset, bytes) in [(129, 8), (138, 8)] {
        let mut invalid = source.clone();
        invalid[offset..offset + bytes].fill(0);
        assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::Identity)));
    }
    for offset in [146, 163] {
        let mut invalid = source.clone();
        invalid[offset] = 2;
        assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::OutputShape)));
    }
    let mut invalid = source.clone();
    invalid[147..155].fill(0);
    assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::OutputShape)));
    let mut invalid = source;
    invalid[164..168].copy_from_slice(&32769u32.to_le_bytes());
    assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::OutputShape)));
    Ok(())
}

#[test]
fn oversized_output_preflight_leaves_storage_unchanged() -> TestResult {
    let bytes = vec![1; MAX_BATCH_BYTES + 1];
    let payload = WirePayload::Reply(ReplyOutcome::OutputDelivery(delivery(&bytes)?));
    let mut output = vec![0x72; MAX_FRAME_BYTES];
    assert!(matches!(
        FrameCodec::encode(&WireMessage { header: header(11)?, payload }, &mut output),
        Err(ProtocolError::OutputShape)
    ));
    assert!(output.iter().all(|byte| *byte == 0x72));
    Ok(())
}
