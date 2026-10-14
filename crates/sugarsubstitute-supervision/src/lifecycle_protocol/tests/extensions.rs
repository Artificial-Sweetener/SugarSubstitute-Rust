// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check exact wire2 main, raw Windows, version and output shapes without native authority.
use super::*;
use std::num::{NonZeroU32, NonZeroU64};

pub(super) fn context() -> Result<GrantedContext<'static>, Box<dyn Error>> {
    Ok(GrantedContext {
        lease: lease()?,
        identity: main()?,
        origin: LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?,
        frontend: VersionLabel::new("1.2.3")?,
        sugarcubes: VersionLabel::new("0.1+ok")?,
    })
}
pub(super) fn failure() -> Result<MainStageFailure, Box<dyn Error>> {
    Ok(MainStageFailure {
        identity: main()?,
        stage: MainStage::Environment,
        failure: MainFailureCode::ValidationFailed,
    })
}
pub(super) fn assert_rejected_tail_and_prefix(source: &[u8]) -> TestResult {
    for length in HEADER_BYTES..source.len() {
        let mut prefix = source[..length].to_vec();
        set_length(&mut prefix);
        assert!(FrameCodec::decode(&prefix).is_err(), "accepted prefix {length}");
    }
    let mut tail = source.to_vec();
    tail.push(0);
    set_length(&mut tail);
    assert!(matches!(FrameCodec::decode(&tail), Err(ProtocolError::TrailingBytes)));
    Ok(())
}

#[test]
fn new_main_payloads_have_exact_stable_kinds_and_full_identity_shapes() -> TestResult {
    let identity = main()?;
    let units = RawWindowsUnits::from_units(&[69, 58, 92, 0xD800])?;
    let fragment = FolderFragment::new(identity, 4, 0, units)?;
    let pid = NonZeroU32::new(53).ok_or("zero PID")?;
    for (payload, kind, body) in [
        (WirePayload::RequestedProfileFolder(fragment), 12, 36),
        (WirePayload::EnvironmentPrepared(fragment), 13, 36),
        (WirePayload::ControllerPrepared { identity, browser_process_id: pid }, 14, 20),
        (WirePayload::MainRetired(identity), 15, 16),
        (
            WirePayload::NativeActivity {
                revision: NonZeroU64::new(59).ok_or("zero revision")?,
                activity: UiWatchdogActivity::Suspended,
            },
            16,
            9,
        ),
        (
            WirePayload::OutputPull { lease: lease()?, acknowledged_delivery: NonZeroU64::new(61) },
            17,
            32,
        ),
        (WirePayload::MainFailed(failure()?), 18, 18),
    ] {
        let source = encoded(payload)?;
        assert_eq!(source.len(), HEADER_BYTES + body);
        assert_eq!(u16::from_le_bytes([source[12], source[13]]), kind);
        assert_eq!(FrameCodec::decode(&source)?.header, header(kind)?);
        match FrameCodec::decode(&source)?.payload {
            DecodedPayload::RequestedProfileFolder(actual)
            | DecodedPayload::EnvironmentPrepared(actual) => assert_eq!(actual, fragment),
            DecodedPayload::ControllerPrepared { identity: actual, browser_process_id } => {
                assert_eq!(actual, identity);
                assert_eq!(browser_process_id, pid);
            }
            DecodedPayload::MainRetired(actual) => assert_eq!(actual, identity),
            DecodedPayload::NativeActivity { revision, activity } => {
                assert_eq!(revision.get(), 59);
                assert_eq!(activity, UiWatchdogActivity::Suspended);
            }
            DecodedPayload::OutputPull { lease: actual, acknowledged_delivery } => {
                assert_eq!(actual, lease()?);
                assert_eq!(acknowledged_delivery.map(NonZeroU64::get), Some(61));
            }
            DecodedPayload::MainFailed(actual) => assert_eq!(actual, failure()?),
            _ => return Err("unexpected extension payload".into()),
        }
        assert_rejected_tail_and_prefix(&source)?;
        for offset in [80, 88] {
            let mut invalid = source.clone();
            invalid[offset..offset + 8].fill(0);
            assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::Identity)));
        }
    }
    Ok(())
}

#[test]
fn exact_two_fragment_maximum_preserves_unpaired_surrogates_and_borrowed_units() -> TestResult {
    let full = vec![0xD800; MAX_PROFILE_FOLDER_UNITS];
    let mut observed = Vec::new();
    for offset in [0, MAX_FOLDER_FRAGMENT_UNITS] {
        let count = (full.len() - offset).min(MAX_FOLDER_FRAGMENT_UNITS);
        let raw = RawWindowsUnits::from_units(&full[offset..offset + count])?;
        let fragment = FolderFragment::new(main()?, full.len() as u32, offset as u32, raw)?;
        let source = encoded(WirePayload::EnvironmentPrepared(fragment))?;
        assert_eq!(source.len(), 132 + count * 2);
        assert!(source.len() <= MAX_FRAME_BYTES);
        let DecodedPayload::EnvironmentPrepared(decoded) = FrameCodec::decode(&source)?.payload
        else {
            return Err("missing folder".into());
        };
        assert_eq!(decoded, fragment);
        assert_eq!(decoded.units(), RawWindowsUnits::from_le_bytes(&source[132..])?);
        observed.extend(decoded.units().units());
    }
    assert_eq!(observed, full);
    Ok(())
}

#[test]
fn raw_units_and_noncanonical_partitions_reject_without_unicode_substitution() -> TestResult {
    for bytes in [&[][..], &[1][..], &[0, 0][..]] {
        assert!(RawWindowsUnits::from_le_bytes(bytes).is_err());
    }
    assert!(RawWindowsUnits::from_units(&vec![1; MAX_FOLDER_FRAGMENT_UNITS + 1]).is_err());
    let units = RawWindowsUnits::from_units(&[0xDC00])?;
    assert!(units.units().eq([0xDC00]));
    for (total, offset) in [(0, 0), (32767, 0), (1, 1), (1, 16384), (32766, 32768), (16385, 0)] {
        assert!(matches!(
            FolderFragment::new(main()?, total, offset, units),
            Err(ProtocolError::FragmentShape)
        ));
    }
    let source =
        encoded(WirePayload::EnvironmentPrepared(FolderFragment::new(main()?, 1, 0, units)?))?;
    for (offset, value) in
        [(120, 0u32), (120, 32767), (124, 1), (124, 32768), (128, 0), (128, 16385)]
    {
        let mut invalid = source.clone();
        invalid[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::FragmentShape)));
    }
    let mut invalid = source;
    invalid[132..134].fill(0);
    assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::InteriorNul)));
    Ok(())
}

#[test]
fn enriched_grant_has_independent_exact_golden_and_borrowed_labels() -> TestResult {
    let value = context()?;
    let source = encoded(WirePayload::Reply(ReplyOutcome::Granted(value)))?;
    let mut golden = vec![3];
    for number in [17u64, 19, 23, 37, 41] {
        golden.extend(number.to_le_bytes());
    }
    golden.push(1);
    golden.extend(8188u16.to_le_bytes());
    for label in [b"1.2.3".as_slice(), b"0.1+ok".as_slice()] {
        golden.push(label.len() as u8);
        golden.extend(label);
    }
    assert_eq!(&source[HEADER_BYTES..], golden);
    let DecodedPayload::Reply(ReplyOutcome::Granted(decoded)) =
        FrameCodec::decode(&source)?.payload
    else {
        return Err("missing enriched grant".into());
    };
    assert_eq!(decoded, value);
    assert_eq!(decoded.frontend.as_str().as_ptr(), source[149..].as_ptr());
    assert_rejected_tail_and_prefix(&source)?;
    for host in ["127.0.0.1", "[::1]", "localhost"] {
        for port in [1, 80, 65535] {
            let origin = LoopbackOrigin::parse(&format!("http://{host}:{port}"), port)?;
            let value = GrantedContext { origin, ..value };
            let source = encoded(WirePayload::Reply(ReplyOutcome::Granted(value)))?;
            assert!(
                matches!(FrameCodec::decode(&source)?.payload,DecodedPayload::Reply(ReplyOutcome::Granted(actual)) if actual==value)
            );
        }
    }
    let mut legacy = source[..129].to_vec();
    set_length(&mut legacy);
    assert!(matches!(FrameCodec::decode(&legacy), Err(ProtocolError::Truncated)));
    Ok(())
}

#[test]
fn shared_version_bound_grammar_and_wire_labels_reject_before_grant() -> TestResult {
    for text in ["", "a b", "é", "a/b", "a\0b", &"a".repeat(65)] {
        assert!(matches!(VersionLabel::new(text), Err(ProtocolError::VersionLabel)));
    }
    let text = "a".repeat(64);
    let value = GrantedContext { frontend: VersionLabel::new(&text)?, ..context()? };
    let source = encoded(WirePayload::Reply(ReplyOutcome::Granted(value)))?;
    assert!(FrameCodec::decode(&source).is_ok());
    for length in [0, 65, 255] {
        let mut invalid = source.clone();
        invalid[148] = length;
        assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::VersionLabel)));
    }
    for byte in [0, b' ', b'/', 0xFF] {
        let mut invalid = source.clone();
        invalid[149] = byte;
        assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::VersionLabel)));
    }
    let mut invalid = source;
    invalid[146..148].fill(0);
    assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::Configuration(_))));
    Ok(())
}

#[test]
fn all_stage_authorization_reply_tags_and_failure_codes_are_explicit() -> TestResult {
    let identity = main()?;
    for (reply, tag) in [
        (ReplyOutcome::EnvironmentAuthorized(identity), 9),
        (
            ReplyOutcome::BrowserAuthorized {
                identity,
                browser_process_id: NonZeroU32::new(53).ok_or("zero PID")?,
            },
            10,
        ),
        (ReplyOutcome::FolderChunkAccepted { identity, next_offset_units: 16384 }, 11),
        (ReplyOutcome::OutputIdle(lease()?), 13),
        (ReplyOutcome::MainStageFailed(failure()?), 14),
    ] {
        let source = encoded(WirePayload::Reply(reply))?;
        assert_eq!(source[104], tag);
        assert!(
            matches!(FrameCodec::decode(&source)?.payload,DecodedPayload::Reply(actual) if actual==reply)
        );
        assert_rejected_tail_and_prefix(&source)?;
    }
    for (stage, stage_tag) in [
        (MainStage::Environment, 1),
        (MainStage::Browser, 2),
        (MainStage::Navigation, 3),
        (MainStage::Frontend, 4),
        (MainStage::NativeRetirement, 5),
    ] {
        for (failure, failure_tag) in [
            (MainFailureCode::ValidationFailed, 1),
            (MainFailureCode::IdentityMismatch, 2),
            (MainFailureCode::OutsideOwnedFamily, 3),
            (MainFailureCode::Unavailable, 4),
            (MainFailureCode::Deadline, 5),
            (MainFailureCode::NativeCleanupUncertain, 6),
        ] {
            let value = MainStageFailure { identity, stage, failure };
            let source = encoded(WirePayload::MainFailed(value))?;
            assert_eq!(&source[120..122], &[stage_tag, failure_tag]);
            assert!(
                matches!(FrameCodec::decode(&source)?.payload,DecodedPayload::MainFailed(actual) if actual==value)
            );
        }
    }
    Ok(())
}

#[test]
fn native_activity_and_output_pull_are_shape_only_and_do_not_synthesize_privilege() -> TestResult {
    for (activity, tag) in [
        (UiWatchdogActivity::VisibleActive, 1),
        (UiWatchdogActivity::Inactive, 2),
        (UiWatchdogActivity::Suspended, 3),
    ] {
        let source = encoded(WirePayload::NativeActivity {
            revision: NonZeroU64::new(1).ok_or("zero revision")?,
            activity,
        })?;
        assert_eq!(source[112], tag);
        assert!(
            matches!(FrameCodec::decode(&source)?.payload,DecodedPayload::NativeActivity {activity:actual,..} if actual==activity)
        );
        let mut invalid = source.clone();
        invalid[112] = 4;
        assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::UnsupportedTag)));
        let mut invalid = source;
        invalid[104..112].fill(0);
        assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::Identity)));
    }
    for acknowledged_delivery in [None, NonZeroU64::new(u64::MAX)] {
        let source = encoded(WirePayload::OutputPull { lease: lease()?, acknowledged_delivery })?;
        assert!(
            matches!(FrameCodec::decode(&source)?.payload,DecodedPayload::OutputPull {acknowledged_delivery:actual,..} if actual==acknowledged_delivery)
        );
    }
    Ok(())
}

#[test]
fn settlement_wire_preserves_original_acquisition_and_rejects_zero_foreign_or_changed_shape()
-> TestResult {
    let identity = main()?;
    let acquisition = sugarsubstitute_contracts::lifecycle::RequestId::new(71)?;
    let source = encoded(WirePayload::SettleAttempt { identity, acquisition })?;
    assert_eq!(source.len(), HEADER_BYTES + 24);
    assert_eq!(&source[4..6], &3u16.to_le_bytes());
    assert!(
        matches!(FrameCodec::decode(&source)?.payload, DecodedPayload::SettleAttempt { identity: actual, acquisition: original } if actual == identity && original == acquisition)
    );
    assert_rejected_tail_and_prefix(&source)?;
    for offset in [104, 112, 120] {
        let mut invalid = source.clone();
        invalid[offset..offset + 8].fill(0);
        assert!(matches!(FrameCodec::decode(&invalid), Err(ProtocolError::Identity)));
    }
    let mut foreign = identity;
    foreign.run = header(19)?.run;
    foreign.run.instance = sugarsubstitute_contracts::lifecycle::InstanceId::new(99)?;
    let mut unchanged = [0x55; 256];
    assert!(
        FrameCodec::encode(
            &WireMessage {
                header: header(19)?,
                payload: WirePayload::SettleAttempt { identity: foreign, acquisition }
            },
            &mut unchanged
        )
        .is_err()
    );
    assert_eq!(unchanged, [0x55; 256]);
    Ok(())
}
