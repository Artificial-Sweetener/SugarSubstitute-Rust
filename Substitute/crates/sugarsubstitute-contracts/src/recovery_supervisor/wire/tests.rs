// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject unknown purposes, malformed original bounds, identity substitutions and secret diagnostics.
use super::*;
use crate::{incident::*, lifecycle::*};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn key() -> Result<RecoveryGrantKey, Box<dyn std::error::Error>> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let run = RunKey { owner, instance: InstanceId::new(7)?, generation: RunGeneration::new(11)? };
    let report = ReportBinding::new(
        IncidentKey::new(IncidentSubject::Desktop(run)),
        IncidentRevision::new(5)?,
        ReportSessionId::new(9)?,
    );
    Ok(RecoveryGrantKey::new(RecoveryGrantId::new(17)?, run, report)?)
}
#[test]
fn every_closed_recovery_command_round_trips_in_its_exact_authenticated_direction() -> TestResult {
    let key = key()?;
    let process = RecoveryProcessIdentity::new(101, 201)?;
    let handler = RecoveryProcessIdentity::new(102, 202)?;
    let commands = [
        (RecoveryCommand::Hello([7; 32]), RecoveryReceiver::Coordinator),
        (
            RecoveryCommand::Startup { total: 3, offset: 0, bytes: b"abc" },
            RecoveryReceiver::Supervisor,
        ),
        (
            RecoveryCommand::Commit(DurableAcknowledgement::new(
                key.report().incident(),
                key.report().revision().next()?,
                IncidentUtc::from_unix_seconds(100)?,
            )),
            RecoveryReceiver::Supervisor,
        ),
        (
            RecoveryCommand::CaptureRoot {
                identity: [9; 24],
                total: 2,
                offset: 0,
                units_le: &[65, 0],
            },
            RecoveryReceiver::Supervisor,
        ),
        (RecoveryCommand::StartupAccepted, RecoveryReceiver::Coordinator),
        (
            RecoveryCommand::CaptureRegistered { supervisor: process, handler },
            RecoveryReceiver::Coordinator,
        ),
        (RecoveryCommand::DesktopCreated(process), RecoveryReceiver::Coordinator),
        (RecoveryCommand::DesktopEnrolled(process), RecoveryReceiver::Supervisor),
        (RecoveryCommand::DesktopExited { process, status: 259 }, RecoveryReceiver::Coordinator),
        (RecoveryCommand::Stop(CounterDeadline::new(100, 1000)?), RecoveryReceiver::Supervisor),
        (RecoveryCommand::Retired, RecoveryReceiver::Coordinator),
        (RecoveryCommand::Failed(RecoveryFailureCode::Capture), RecoveryReceiver::Coordinator),
    ];
    for (index, (command, receiver)) in commands.into_iter().enumerate() {
        let frame = RecoveryFrame { key, sequence: index as u64 + 1, command };
        let mut first = [0; RECOVERY_WIRE_BYTES];
        let length = encode(&frame, receiver, &mut first)?;
        let actual = decode(&first[..length], receiver)?;
        assert_eq!(actual.key, key);
        assert_eq!(actual.sequence, frame.sequence);
        let mut second = [0; RECOVERY_WIRE_BYTES];
        assert_eq!(encode(&actual, receiver, &mut second)?, length);
        assert_eq!(&first[..length], &second[..length]);
        if !matches!(actual.command, RecoveryCommand::Failed(_)) {
            let wrong = match receiver {
                RecoveryReceiver::Coordinator => RecoveryReceiver::Supervisor,
                RecoveryReceiver::Supervisor => RecoveryReceiver::Coordinator,
            };
            assert_eq!(decode(&first[..length], wrong).err(), Some(RecoveryWireError::Direction));
        }
    }
    Ok(())
}
#[test]
fn schema_reserved_extents_unknown_commands_and_changed_grants_fail_closed() -> TestResult {
    let frame =
        RecoveryFrame { key: key()?, sequence: 1, command: RecoveryCommand::Hello([7; 32]) };
    let mut encoded = [0; RECOVERY_WIRE_BYTES];
    let length = encode(&frame, RecoveryReceiver::Coordinator, &mut encoded)?;
    for (at, value) in [
        (0, b'X'),
        (4, 2),
        (6, 0),
        (8, 99),
        (12, 31),
        (16, 0),
        (24, 0),
        (24 + 40, 0),
        (24 + 124, 1),
        (184, 1),
        (RECOVERY_HEADER_BYTES, 0),
    ] {
        let mut bad = encoded;
        bad[at] = value;
        if at == 16 {
            bad[16..24].fill(0);
        }
        if at == 24 {
            bad[24..32].fill(0);
        }
        if at == 24 + 40 {
            bad[at..at + 16].fill(0);
        }
        if at == RECOVERY_HEADER_BYTES {
            bad[at..at + 32].fill(0);
        }
        assert!(decode(&bad[..length], RecoveryReceiver::Coordinator).is_err(), "field {at}");
    }
    assert!(decode(&encoded[..length - 1], RecoveryReceiver::Coordinator).is_err());
    assert!(decode(&encoded[..length + 1], RecoveryReceiver::Coordinator).is_err());
    let mut foreign = encoded;
    foreign[24 + 72] ^= 1;
    assert_eq!(
        decode(&foreign[..length], RecoveryReceiver::Coordinator).err(),
        Some(RecoveryWireError::Binding)
    );
    Ok(())
}
#[test]
fn startup_fragment_bounds_are_exact_and_rejection_preserves_destination() -> TestResult {
    let key = key()?;
    let payload = [7; RECOVERY_STARTUP_CHUNK];
    let valid = RecoveryFrame {
        key,
        sequence: 1,
        command: RecoveryCommand::Startup {
            total: RECOVERY_STARTUP_BYTES as u32,
            offset: 0,
            bytes: &payload,
        },
    };
    let mut bytes = [0; RECOVERY_WIRE_BYTES];
    assert_eq!(encode(&valid, RecoveryReceiver::Supervisor, &mut bytes)?, RECOVERY_WIRE_BYTES);
    assert!(decode(&bytes, RecoveryReceiver::Supervisor).is_ok());
    for (total, offset, body) in [
        (0, 0, b"x".as_slice()),
        (1, 1, b"x"),
        (RECOVERY_STARTUP_BYTES as u32 + 1, 0, b"x"),
        (1, 0, b""),
        (1, u32::MAX, b"x"),
    ] {
        bytes.fill(0x6d);
        let bad = RecoveryFrame {
            key,
            sequence: 1,
            command: RecoveryCommand::Startup { total, offset, bytes: body },
        };
        assert_eq!(
            encode(&bad, RecoveryReceiver::Supervisor, &mut bytes),
            Err(RecoveryWireError::Capacity)
        );
        assert!(bytes.iter().all(|value| *value == 0x6d));
    }
    Ok(())
}
#[test]
fn acknowledgement_and_process_pair_substitutions_reject_before_encoding() -> TestResult {
    let key = key()?;
    let ack = DurableAcknowledgement::new(
        key.report().incident(),
        key.report().revision(),
        IncidentUtc::from_unix_seconds(100)?,
    );
    let frame = RecoveryFrame { key, sequence: 1, command: RecoveryCommand::Commit(ack) };
    let mut bytes = [0x6d; RECOVERY_WIRE_BYTES];
    assert_eq!(
        encode(&frame, RecoveryReceiver::Supervisor, &mut bytes),
        Err(RecoveryWireError::Binding)
    );
    assert!(bytes.iter().all(|value| *value == 0x6d));
    let process = RecoveryProcessIdentity::new(101, 201)?;
    let frame = RecoveryFrame {
        key,
        sequence: 1,
        command: RecoveryCommand::CaptureRegistered { supervisor: process, handler: process },
    };
    assert_eq!(
        encode(&frame, RecoveryReceiver::Coordinator, &mut bytes),
        Err(RecoveryWireError::Binding)
    );
    assert!(RecoveryGrantId::new(0).is_err());
    assert!(RecoveryProcessIdentity::new(0, 1).is_err());
    assert!(RecoveryProcessIdentity::new(1, 0).is_err());
    Ok(())
}
#[test]
fn recovery_debug_never_contains_session_capabilities_or_original_launch_bytes() -> TestResult {
    let frame =
        RecoveryFrame { key: key()?, sequence: 1, command: RecoveryCommand::Hello([0xab; 32]) };
    assert_eq!(format!("{frame:?}"), "RecoveryFrame { redacted }");
    let frame = RecoveryFrame {
        key: key()?,
        sequence: 1,
        command: RecoveryCommand::Startup {
            total: 23,
            offset: 0,
            bytes: b"private-launch-payload!",
        },
    };
    assert_eq!(format!("{frame:?}"), "RecoveryFrame { redacted }");
    Ok(())
}

#[test]
fn configured_capture_root_requires_nonzero_identity_even_bounded_utf16_and_original_extent()
-> TestResult {
    let key = key()?;
    for (identity, total, offset, units) in [
        ([0; 24], 2, 0, &[65, 0][..]),
        ([1; 24], 3, 0, &[65, 0][..]),
        ([1; 24], 4, 1, &[65, 0][..]),
        ([1; 24], 4, 0, &[65][..]),
        ([1; 24], 4, 0, &[0, 0][..]),
        ([1; 24], RECOVERY_ROOT_BYTES as u32 + 2, 0, &[65, 0][..]),
    ] {
        let mut output = [0x7b; RECOVERY_WIRE_BYTES];
        let before = output;
        let frame = RecoveryFrame {
            key,
            sequence: 1,
            command: RecoveryCommand::CaptureRoot { identity, total, offset, units_le: units },
        };
        assert!(encode(&frame, RecoveryReceiver::Supervisor, &mut output).is_err());
        assert_eq!(output, before);
    }
    let root = [65; RECOVERY_ROOT_CHUNK];
    let mut output = [0; RECOVERY_WIRE_BYTES];
    let frame = RecoveryFrame {
        key,
        sequence: 1,
        command: RecoveryCommand::CaptureRoot {
            identity: [1; 24],
            total: RECOVERY_ROOT_BYTES as u32,
            offset: 0,
            units_le: &root,
        },
    };
    assert_eq!(encode(&frame, RecoveryReceiver::Supervisor, &mut output)?, RECOVERY_WIRE_BYTES);
    assert!(decode(&output, RecoveryReceiver::Supervisor).is_ok());
    Ok(())
}

#[test]
fn direct_desktop_enrollment_reply_has_exact_identity_and_relay_commands_remain_unsupported()
-> TestResult {
    let process = RecoveryProcessIdentity::new(101, 201)?;
    let frame = RecoveryFrame {
        key: key()?,
        sequence: 2,
        command: RecoveryCommand::DesktopEnrolled(process),
    };
    let mut encoded = [0; RECOVERY_WIRE_BYTES];
    assert_eq!(
        encode(&frame, RecoveryReceiver::Coordinator, &mut encoded),
        Err(RecoveryWireError::Direction)
    );
    let length = encode(&frame, RecoveryReceiver::Supervisor, &mut encoded)?;
    assert!(
        matches!(decode(&encoded[..length],RecoveryReceiver::Supervisor)?.command,RecoveryCommand::DesktopEnrolled(actual) if actual==process)
    );
    for kind in [9u32, 10, 15] {
        let mut unsupported = encoded;
        unsupported[8..12].copy_from_slice(&kind.to_le_bytes());
        assert_eq!(
            decode(&unsupported[..length], RecoveryReceiver::Supervisor).err(),
            Some(RecoveryWireError::Malformed)
        );
        assert_eq!(
            decode(&unsupported[..length], RecoveryReceiver::Coordinator).err(),
            Some(RecoveryWireError::Malformed)
        );
    }
    for range in [0..4, 4..8, 8..16] {
        let mut bad = encoded;
        let start = RECOVERY_HEADER_BYTES + range.start;
        let end = RECOVERY_HEADER_BYTES + range.end;
        if range.start == 4 {
            bad[start] = 1;
        } else {
            bad[start..end].fill(0);
        }
        assert!(decode(&bad[..length], RecoveryReceiver::Supervisor).is_err());
    }
    Ok(())
}

#[path = "tests/recovery_additions.rs"]
mod recovery_additions;
