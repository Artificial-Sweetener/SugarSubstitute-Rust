// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check stable operation/reply tags and redacted borrowed secret ownership.
use super::*;

#[test]
fn all_fixed_operations_roundtrip_with_stable_kind_and_payload_lengths() -> TestResult {
    let reference = lease()?;
    let shutdown = ShutdownId::new(29)?;
    let challenge = UiChallengeId::new(31)?;
    for (payload, kind, bytes) in [
        (WirePayload::Hello(CapabilityRef::new(&[0xAB; 32])), 1, 32),
        (WirePayload::Welcome { qpc_frequency: 10_000_000 }, 2, 8),
        (WirePayload::Register, 3, 0),
        (WirePayload::Release(reference), 5, 24),
        (WirePayload::Reload { lease: reference, identity: main()? }, 6, 40),
        (
            WirePayload::ExitIntent {
                shutdown,
                intent: CompletionIntent::StartupFailed,
                expected_code: u32::MAX,
            },
            7,
            13,
        ),
        (WirePayload::ExitCompleted(shutdown), 8, 8),
        (WirePayload::UiChallenge(challenge), 9, 8),
        (WirePayload::UiResponse(challenge), 10, 8),
        (WirePayload::Reply(ReplyOutcome::Acknowledged), 11, 1),
    ] {
        let source = encoded(payload)?;
        assert_eq!(source.len(), HEADER_BYTES + bytes);
        assert_eq!(u16::from_le_bytes([source[12], source[13]]), kind);
        assert_eq!(FrameCodec::decode(&source)?.header, header(kind)?);
        for length in HEADER_BYTES..source.len() {
            let mut prefix = source[..length].to_vec();
            set_length(&mut prefix);
            assert!(matches!(FrameCodec::decode(&prefix), Err(ProtocolError::Truncated)));
        }
        let mut trailing = source.clone();
        trailing.push(0);
        set_length(&mut trailing);
        assert!(matches!(FrameCodec::decode(&trailing), Err(ProtocolError::TrailingBytes)));
        for offset in [80, 88] {
            let mut invalid = source.clone();
            invalid[offset..offset + 8].fill(0);
            if source[offset..offset + 8].iter().all(|byte| *byte == 0) {
                invalid[offset] = 1;
            }
            assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::Identity)));
        }
        match FrameCodec::decode(&source)?.payload {
            DecodedPayload::Hello(secret) => assert_eq!(secret.bytes(), &[0xAB; 32]),
            DecodedPayload::Welcome { qpc_frequency } => assert_eq!(qpc_frequency, 10_000_000),
            DecodedPayload::Register => assert_eq!(kind, 3),
            DecodedPayload::Release(value) | DecodedPayload::Reload { lease: value, .. } => {
                assert_eq!(value, reference)
            }
            DecodedPayload::ExitIntent { shutdown: id, intent, expected_code } => {
                assert_eq!(id, shutdown);
                assert_eq!(intent, CompletionIntent::StartupFailed);
                assert_eq!(expected_code, u32::MAX);
            }
            DecodedPayload::ExitCompleted(value) => assert_eq!(value, shutdown),
            DecodedPayload::UiChallenge(value) | DecodedPayload::UiResponse(value) => {
                assert_eq!(value, challenge)
            }
            DecodedPayload::Reply(value) => assert_eq!(value, ReplyOutcome::Acknowledged),
            _ => return Err("Unexpected target operation".into()),
        }
    }
    for (intent, tag) in [
        (CompletionIntent::Normal, 1),
        (CompletionIntent::StartupCancelled, 2),
        (CompletionIntent::StartupFailed, 3),
    ] {
        let source = encoded(WirePayload::ExitIntent { shutdown, intent, expected_code: 259 })?;
        assert_eq!(source[112], tag);
        assert!(matches!(
            FrameCodec::decode(&source)?.payload,
            DecodedPayload::ExitIntent { intent: actual, expected_code: 259, .. } if actual == intent
        ));
    }
    Ok(())
}
#[test]
fn every_reply_classification_has_explicit_stable_tags_and_exact_roundtrip() -> TestResult {
    let reference = lease()?;
    let mut outcomes = vec![
        (ReplyOutcome::Acknowledged, vec![1]),
        (ReplyOutcome::Released, vec![4]),
        (ReplyOutcome::Cancelled, vec![6]),
    ];
    let mut pending = vec![2];
    for value in [17u64, 19, 23] {
        pending.extend(value.to_le_bytes());
    }
    outcomes.push((ReplyOutcome::Pending(reference), pending));
    for (failure, tag) in [
        (LifecycleFailureCode::Preparation, 1),
        (LifecycleFailureCode::Cleanup, 2),
        (LifecycleFailureCode::UnexpectedExit, 3),
        (LifecycleFailureCode::Deadline, 4),
    ] {
        outcomes.push((ReplyOutcome::Failed(failure), vec![7, tag]));
    }
    for (rejection, tag) in [
        (LifecycleRejection::ForeignOwner, 1),
        (LifecycleRejection::StaleIdentity, 2),
        (LifecycleRejection::ProfileContention, 3),
        (LifecycleRejection::WrongPhase, 4),
        (LifecycleRejection::ChangedDuplicate, 5),
        (LifecycleRejection::StaleRequest, 6),
        (LifecycleRejection::Overloaded, 7),
        (LifecycleRejection::IdentityExhausted, 8),
        (LifecycleRejection::InvalidClock, 9),
        (LifecycleRejection::EndpointMismatch, 10),
    ] {
        outcomes.push((ReplyOutcome::Rejected(rejection), vec![8, tag]));
    }
    for (outcome, golden) in outcomes {
        let source = encoded(WirePayload::Reply(outcome))?;
        assert_eq!(&source[HEADER_BYTES..], golden);
        assert!(
            matches!(FrameCodec::decode(&source)?.payload, DecodedPayload::Reply(actual) if actual == outcome)
        );
    }
    Ok(())
}
#[test]
fn unknown_reply_intent_and_zero_nested_identity_fail_closed() -> TestResult {
    for (payload, offset, replacement) in [
        (WirePayload::Reply(ReplyOutcome::Acknowledged), 104, u8::MAX),
        (WirePayload::Reply(ReplyOutcome::Failed(LifecycleFailureCode::Deadline)), 105, 5),
        (WirePayload::Reply(ReplyOutcome::Rejected(LifecycleRejection::EndpointMismatch)), 105, 11),
        (
            WirePayload::ExitIntent {
                shutdown: ShutdownId::new(1)?,
                intent: CompletionIntent::Normal,
                expected_code: 259,
            },
            112,
            4,
        ),
    ] {
        let mut source = encoded(payload)?;
        source[offset] = replacement;
        assert!(matches!(FrameCodec::decode(&source), Err(ProtocolError::UnsupportedTag)));
    }
    for payload in [
        WirePayload::Release(lease()?),
        WirePayload::ExitCompleted(ShutdownId::new(1)?),
        WirePayload::UiResponse(UiChallengeId::new(1)?),
    ] {
        let mut source = encoded(payload)?;
        source[104..112].fill(0);
        assert!(matches!(FrameCodec::decode(&source), Err(ProtocolError::Identity)));
    }
    Ok(())
}
#[test]
fn capability_is_borrowed_and_all_debug_paths_redact_its_bytes() -> TestResult {
    let secret = *b"capability-secret-do-not-print!!";
    let source = encoded(WirePayload::Hello(CapabilityRef::new(&secret)))?;
    let decoded = FrameCodec::decode(&source)?;
    let DecodedPayload::Hello(capability) = decoded.payload else {
        return Err("Expected capability".into());
    };
    assert_eq!(capability.bytes().as_ptr(), source[104..].as_ptr());
    for formatted in [
        format!("{capability:?}"),
        format!("{:?}", decoded.payload),
        format!("{decoded:?}"),
        format!(
            "{:?}",
            WireMessage {
                header: header(1)?,
                payload: WirePayload::Hello(CapabilityRef::new(&secret))
            }
        ),
    ] {
        assert!(formatted.contains("REDACTED"));
        assert!(!formatted.contains("capability-secret"));
        assert!(!formatted.contains("115, 101, 110"));
    }
    Ok(())
}

#[test]
fn reloaded_context_has_exact_new_identity_origin_versions_and_rejects_lease_only_payload()
-> TestResult {
    let run = header(11)?.run;
    let context = GrantedContext {
        lease: lease()?,
        identity: MainIdentity { run, launch: LaunchId::new(2)?, navigation: Generation::new(3)? },
        origin: LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?,
        frontend: VersionLabel::new("1.47.11")?,
        sugarcubes: VersionLabel::new("0.15.3")?,
        backend_peer: extensions::context()?.backend_peer,
    };
    let bytes = encoded(WirePayload::Reply(ReplyOutcome::Reloaded(context)))?;
    assert_eq!(bytes[HEADER_BYTES], 5);
    assert!(
        matches!(FrameCodec::decode(&bytes)?.payload,DecodedPayload::Reply(ReplyOutcome::Reloaded(actual)) if actual==context)
    );
    let mut old = bytes[..HEADER_BYTES + 25].to_vec();
    set_length(&mut old);
    assert!(FrameCodec::decode(&old).is_err());
    Ok(())
}
