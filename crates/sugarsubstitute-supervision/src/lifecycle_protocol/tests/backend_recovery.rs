// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify current recovery shapes without granting peer, retry or native adoption authority.
use super::*;

fn replacement() -> Result<LeaseReference, Box<dyn Error>> {
    Ok(LeaseReference {
        resource: lease()?.resource,
        generation: ResourceGeneration::new(u64::MAX)?,
        lease: LeaseId::new(u64::MAX)?,
    })
}
fn frames() -> Result<[Vec<u8>; 3], Box<dyn Error>> {
    Ok([
        encoded(WirePayload::RetryBackend { identity: main()?, failed: lease()? })?,
        encoded(WirePayload::Reply(ReplyOutcome::BackendUnavailable {
            failed: lease()?,
            status: 259,
            affected_clients: 2,
            retry_ready: true,
        }))?,
        encoded(WirePayload::Reply(ReplyOutcome::BackendReplaced {
            identity: main()?,
            previous: lease()?,
            current: replacement()?,
        }))?,
    ])
}
#[test]
fn retry_backend_kind20_has_exact40_byte_payload_and_preserves_large_ids() -> TestResult {
    let identity = MainIdentity {
        launch: LaunchId::new(u64::MAX)?,
        navigation: Generation::new(u64::MAX)?,
        ..main()?
    };
    let failed = replacement()?;
    let source = encoded(WirePayload::RetryBackend { identity, failed })?;
    assert_eq!(source.len(), HEADER_BYTES + 40);
    assert_eq!(&source[4..6], &10u16.to_le_bytes());
    assert_eq!(&source[12..14], &20u16.to_le_bytes());
    let mut golden = Vec::new();
    for value in [u64::MAX, u64::MAX, 17, u64::MAX, u64::MAX] {
        golden.extend(value.to_le_bytes());
    }
    assert_eq!(&source[HEADER_BYTES..], golden);
    let decoded = FrameCodec::decode(&source)?;
    assert_eq!(decoded.header, header(20)?);
    assert!(matches!(decoded.payload,
        DecodedPayload::RetryBackend { identity: actual, failed: original }
        if actual == identity && original == failed));
    Ok(())
}
#[test]
fn backend_unavailable_tag15_has_exact32_byte_payload_and_unsigned_status() -> TestResult {
    let failed = lease()?;
    for status in [0u32, 259, u32::MAX] {
        for affected_clients in [1u16, 64] {
            for retry_ready in [false, true] {
                let reply = ReplyOutcome::BackendUnavailable {
                    failed,
                    status,
                    affected_clients,
                    retry_ready,
                };
                let source = encoded(WirePayload::Reply(reply))?;
                assert_eq!(source.len(), HEADER_BYTES + 32);
                let mut golden = vec![15];
                for value in [17u64, 19, 23] {
                    golden.extend(value.to_le_bytes());
                }
                golden.extend(status.to_le_bytes());
                golden.extend(affected_clients.to_le_bytes());
                golden.push(u8::from(retry_ready));
                assert_eq!(&source[HEADER_BYTES..], golden);
                assert!(matches!(FrameCodec::decode(&source)?.payload,
                    DecodedPayload::Reply(actual) if actual == reply));
            }
        }
    }
    Ok(())
}
#[test]
fn backend_replaced_tag16_has_exact65_byte_payload_and_both_bindings() -> TestResult {
    let reply = ReplyOutcome::BackendReplaced {
        identity: main()?,
        previous: lease()?,
        current: replacement()?,
    };
    let source = encoded(WirePayload::Reply(reply))?;
    assert_eq!(source.len(), HEADER_BYTES + 65);
    let mut golden = vec![16];
    for value in [37u64, 41, 17, 19, 23, 17, u64::MAX, u64::MAX] {
        golden.extend(value.to_le_bytes());
    }
    assert_eq!(&source[HEADER_BYTES..], golden);
    assert_eq!(FrameCodec::decode(&source)?.header, header(11)?);
    assert!(matches!(FrameCodec::decode(&source)?.payload,
        DecodedPayload::Reply(actual) if actual == reply));
    Ok(())
}
#[test]
fn recovery_frames_reject_prior_future_schema_and_unknown_operation_or_reply_tag() -> TestResult {
    for source in frames()? {
        for version in [0u16, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, u16::MAX] {
            let mut invalid = source.clone();
            invalid[4..6].copy_from_slice(&version.to_le_bytes());
            assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::Schema)));
        }
    }
    let [mut request, mut unavailable, mut replaced] = frames()?;
    request[12..14].copy_from_slice(&22u16.to_le_bytes());
    assert!(matches!(FrameCodec::decode(&request), Err(ProtocolError::UnsupportedTag)));
    for source in [&mut unavailable, &mut replaced] {
        source[104] = 17;
        assert!(matches!(FrameCodec::decode(source), Err(ProtocolError::UnsupportedTag)));
    }
    Ok(())
}
#[test]
fn every_recovery_prefix_and_extra_payload_byte_is_rejected() -> TestResult {
    for source in frames()? {
        for length in 0..source.len() {
            let mut prefix = source[..length].to_vec();
            if length >= HEADER_BYTES {
                set_length(&mut prefix);
            }
            assert!(FrameCodec::decode(&prefix).is_err(), "accepted prefix {length}");
        }
        let mut trailing = source;
        trailing.push(0);
        set_length(&mut trailing);
        assert!(matches!(FrameCodec::decode(&trailing), Err(ProtocolError::TrailingBytes)));
    }
    Ok(())
}
#[test]
fn recovery_main_and_lease_coordinates_require_positive_values() -> TestResult {
    let [request, unavailable, replaced] = frames()?;
    for (source, offsets) in [
        (request, vec![104, 112, 120, 128, 136]),
        (unavailable, vec![105, 113, 121]),
        (replaced, vec![105, 113, 121, 129, 137, 145, 153, 161]),
    ] {
        for offset in offsets {
            let mut invalid = source.clone();
            invalid[offset..offset + 8].fill(0);
            assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::Identity)));
        }
    }
    Ok(())
}
#[test]
fn retry_backend_requires_positive_request_connection_and_original_qpc() -> TestResult {
    let source = frames()?[0].clone();
    for offset in [80, 88] {
        let mut invalid = source.clone();
        invalid[offset..offset + 8].fill(0);
        assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::Identity)));
    }
    for deadline in [0i64, -1, i64::MIN] {
        let mut invalid = source.clone();
        invalid[96..104].copy_from_slice(&deadline.to_le_bytes());
        assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::Clock)));
    }
    for missing_request in [false, true] {
        let mut h = header(20)?;
        if missing_request {
            h.request = None;
        } else {
            h.connection = None;
        }
        let mut output = [0x6A; 144];
        assert!(matches!(
            FrameCodec::encode(
                &WireMessage {
                    header: h,
                    payload: WirePayload::RetryBackend { identity: main()?, failed: lease()? },
                },
                &mut output
            ),
            Err(ProtocolError::Identity)
        ));
        assert_eq!(output, [0x6A; 144]);
    }
    Ok(())
}
#[test]
fn unavailable_client_count_and_boolean_are_closed_and_encode_preflight_is_atomic() -> TestResult {
    let source = frames()?[1].clone();
    for count in [0u16, 65, u16::MAX] {
        let mut invalid = source.clone();
        invalid[133..135].copy_from_slice(&count.to_le_bytes());
        assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::Length)));
        let mut output = [0x72; 136];
        let payload = WirePayload::Reply(ReplyOutcome::BackendUnavailable {
            failed: lease()?,
            status: 259,
            affected_clients: count,
            retry_ready: false,
        });
        assert!(matches!(
            FrameCodec::encode(&WireMessage { header: header(11)?, payload }, &mut output),
            Err(ProtocolError::Length)
        ));
        assert_eq!(output, [0x72; 136]);
    }
    for flag in 2u8..=u8::MAX {
        let mut invalid = source.clone();
        invalid[135] = flag;
        assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::UnsupportedTag)));
    }
    Ok(())
}
#[test]
fn recovery_main_full_run_mismatch_rejects_before_output_mutation() -> TestResult {
    for field in 0..4 {
        let mut identity = main()?;
        match field {
            0 => identity.run.owner.namespace = NamespaceId::new([0x99; 32]),
            1 => identity.run.owner.epoch = CoordinatorEpoch::new([0x99; 16])?,
            2 => identity.run.instance = InstanceId::new(99)?,
            _ => identity.run.generation = RunGeneration::new(99)?,
        }
        for payload in [
            WirePayload::RetryBackend { identity, failed: lease()? },
            WirePayload::Reply(ReplyOutcome::BackendReplaced {
                identity,
                previous: lease()?,
                current: replacement()?,
            }),
        ] {
            let mut output = [0x6A; 169];
            assert!(matches!(
                FrameCodec::encode(
                    &WireMessage { header: header(payload.kind())?, payload },
                    &mut output
                ),
                Err(ProtocolError::Identity)
            ));
            assert_eq!(output, [0x6A; 169]);
        }
    }
    Ok(())
}
#[test]
fn recovery_output_capacity_and_original_frame_cap_remain_exact() -> TestResult {
    let payloads = [
        WirePayload::RetryBackend { identity: main()?, failed: lease()? },
        WirePayload::Reply(ReplyOutcome::BackendUnavailable {
            failed: lease()?,
            status: 259,
            affected_clients: 1,
            retry_ready: false,
        }),
        WirePayload::Reply(ReplyOutcome::BackendReplaced {
            identity: main()?,
            previous: lease()?,
            current: replacement()?,
        }),
    ];
    for (payload, required) in payloads.into_iter().zip([144usize, 136, 169]) {
        let mut output = vec![0x6A; required - 1];
        assert!(matches!(FrameCodec::encode(&WireMessage {
            header: header(payload.kind())?, payload,
        }, &mut output), Err(ProtocolError::OutputTooSmall { required: actual, available })
            if actual == required && available == required - 1));
        assert!(output.iter().all(|byte| *byte == 0x6A));
    }
    assert_eq!(HEADER_BYTES, 104);
    assert_eq!(MAX_FRAME_BYTES, 65_536);
    let mut oversized = frames()?[2].clone();
    oversized.resize(MAX_FRAME_BYTES + 1, 0);
    set_length(&mut oversized);
    assert!(matches!(FrameCodec::decode(&oversized), Err(ProtocolError::FrameTooLarge)));
    Ok(())
}
