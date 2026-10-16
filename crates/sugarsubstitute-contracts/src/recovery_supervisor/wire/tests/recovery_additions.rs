// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Protect recovery additions at the real bounded codec; session owners separately reject replay.
use super::*;
use crate::{execution::Generation, startup::LaunchId};
#[test]
fn desktop_identity_requires_tag_seventeen_exact_extent_direction_and_positive_fields() -> TestResult
{
    let frame = RecoveryFrame {
        key: key()?,
        sequence: u64::MAX,
        command: RecoveryCommand::DesktopIdentity {
            launch: LaunchId::new(u64::MAX)?,
            navigation: Generation::new(u64::MAX)?,
        },
    };
    let mut bytes = [0x6d; RECOVERY_WIRE_BYTES];
    let length = encode(&frame, RecoveryReceiver::Supervisor, &mut bytes)?;
    assert_eq!(length, RECOVERY_HEADER_BYTES + 16);
    assert_eq!(&bytes[8..12], &17u32.to_le_bytes());
    assert_eq!(&bytes[RECOVERY_HEADER_BYTES..length], &[0xff; 16]);
    assert!(bytes[length..].iter().all(|byte| *byte == 0x6d));
    let actual = decode(&bytes[..length], RecoveryReceiver::Supervisor)?;
    assert_eq!(actual.key, frame.key);
    assert_eq!(actual.sequence, frame.sequence);
    assert!(matches!(actual.command, RecoveryCommand::DesktopIdentity { launch, navigation }
        if launch.get() == u64::MAX && navigation.get() == u64::MAX));
    let mut rejected = [0x6d; RECOVERY_WIRE_BYTES];
    assert_eq!(
        encode(&frame, RecoveryReceiver::Coordinator, &mut rejected),
        Err(RecoveryWireError::Direction)
    );
    assert_eq!(rejected, [0x6d; RECOVERY_WIRE_BYTES]);
    assert_eq!(
        decode(&bytes[..length], RecoveryReceiver::Coordinator).err(),
        Some(RecoveryWireError::Direction)
    );
    for offset in [RECOVERY_HEADER_BYTES, RECOVERY_HEADER_BYTES + 8] {
        let mut malformed = bytes;
        malformed[offset..offset + 8].fill(0);
        assert_eq!(
            decode(&malformed[..length], RecoveryReceiver::Supervisor).err(),
            Some(RecoveryWireError::Binding)
        );
    }
    for body_length in [15usize, 17] {
        let mut malformed = bytes;
        malformed[12..16].copy_from_slice(&(body_length as u32).to_le_bytes());
        assert_eq!(
            decode(&malformed[..RECOVERY_HEADER_BYTES + body_length], RecoveryReceiver::Supervisor)
                .err(),
            Some(RecoveryWireError::Malformed)
        );
    }
    for offset in 184..RECOVERY_HEADER_BYTES {
        let mut malformed = bytes;
        malformed[offset] = 1;
        assert_eq!(
            decode(&malformed[..length], RecoveryReceiver::Supervisor).err(),
            Some(RecoveryWireError::Malformed)
        );
    }
    let mut removed = bytes;
    removed[8..12].copy_from_slice(&7u32.to_le_bytes());
    for receiver in [RecoveryReceiver::Coordinator, RecoveryReceiver::Supervisor] {
        assert_eq!(decode(&removed[..length], receiver).err(), Some(RecoveryWireError::Malformed));
    }
    Ok(())
}
#[test]
fn direct_root_encoder_preserves_exact_maximum_utf16_fragments_and_original_key() -> TestResult {
    let key = key()?;
    let count = RECOVERY_ROOT_CHUNK / 2;
    let mut units = vec![0x41; RECOVERY_ROOT_BYTES / 2];
    units[count - 1] = 0xd800;
    units[count] = 0xdc00;
    units[count + 1] = 0xdfff;
    let mut bytes = [0; RECOVERY_WIRE_BYTES];
    let total = units.len() as u32;
    let (first_length, next) = encode_capture_root_units(
        key,
        1,
        [9; 24],
        total,
        0,
        units[..count].iter().copied(),
        &mut bytes,
    )?;
    assert_eq!(first_length, RECOVERY_WIRE_BYTES);
    assert_eq!(next, count as u32);
    let first = decode(&bytes[..first_length], RecoveryReceiver::Supervisor)?;
    assert_eq!(first.key, key);
    assert_eq!(first.sequence, 1);
    let RecoveryCommand::CaptureRoot { identity, total: actual_total, offset, units_le } =
        first.command
    else {
        return Err("Actual capture-root fragment missing".into());
    };
    assert_eq!(identity, [9; 24]);
    assert_eq!(actual_total, RECOVERY_ROOT_BYTES as u32);
    assert_eq!(offset, 0);
    let decoded: Vec<_> =
        units_le.chunks_exact(2).map(|unit| u16::from_le_bytes([unit[0], unit[1]])).collect();
    assert_eq!(decoded, units[..count]);
    let mut canonical = [0; RECOVERY_WIRE_BYTES];
    let canonical_length = encode(
        &RecoveryFrame {
            key,
            sequence: 1,
            command: RecoveryCommand::CaptureRoot {
                identity,
                total: actual_total,
                offset,
                units_le,
            },
        },
        RecoveryReceiver::Supervisor,
        &mut canonical,
    )?;
    assert_eq!(canonical_length, first_length);
    assert_eq!(canonical, bytes);
    let (last_length, complete) = encode_capture_root_units(
        key,
        2,
        [9; 24],
        total,
        next,
        units[count..].iter().copied(),
        &mut bytes,
    )?;
    assert_eq!(complete, total);
    assert_eq!(last_length, RECOVERY_HEADER_BYTES + 32 + (units.len() - count) * 2);
    let last = decode(&bytes[..last_length], RecoveryReceiver::Supervisor)?;
    assert_eq!(last.key, key);
    assert_eq!(last.sequence, 2);
    let RecoveryCommand::CaptureRoot { total: actual_total, offset, units_le, .. } = last.command
    else {
        return Err("Actual final root fragment missing".into());
    };
    assert_eq!(actual_total, total * 2);
    assert_eq!(offset, next * 2);
    let decoded: Vec<_> =
        units_le.chunks_exact(2).map(|unit| u16::from_le_bytes([unit[0], unit[1]])).collect();
    assert_eq!(decoded, units[count..]);
    assert_eq!(
        decode(&bytes[..last_length], RecoveryReceiver::Coordinator).err(),
        Some(RecoveryWireError::Direction)
    );
    Ok(())
}
#[test]
fn direct_root_encoder_rejects_invalid_extent_before_iteration_and_changed_iterator_without_success()
-> TestResult {
    let key = key()?;
    for (identity, total, offset) in [
        ([0; 24], 1, 0),
        ([9; 24], 0, 0),
        ([9; 24], RECOVERY_ROOT_BYTES as u32 / 2 + 1, 0),
        ([9; 24], 1, 1),
        ([9; 24], 1, u32::MAX),
    ] {
        let calls = std::cell::Cell::new(0);
        let units = std::iter::from_fn(|| {
            calls.set(calls.get() + 1);
            Some(65)
        });
        let mut bytes = [0x6d; RECOVERY_WIRE_BYTES];
        assert_eq!(
            encode_capture_root_units(key, 1, identity, total, offset, units, &mut bytes),
            Err(RecoveryWireError::Binding)
        );
        assert_eq!(calls.get(), 0);
        assert_eq!(bytes, [0x6d; RECOVERY_WIRE_BYTES]);
    }
    for (sequence, capacity) in [(0, RECOVERY_WIRE_BYTES), (1, RECOVERY_HEADER_BYTES + 33)] {
        let calls = std::cell::Cell::new(0);
        let units = std::iter::from_fn(|| {
            calls.set(calls.get() + 1);
            Some(65)
        });
        let mut bytes = [0x6d; RECOVERY_WIRE_BYTES];
        assert_eq!(
            encode_capture_root_units(key, sequence, [9; 24], 1, 0, units, &mut bytes[..capacity]),
            Err(RecoveryWireError::Capacity)
        );
        assert_eq!(calls.get(), 0);
        assert_eq!(bytes, [0x6d; RECOVERY_WIRE_BYTES]);
    }
    for units in [vec![], vec![65, 66], vec![65, 66, 67, 68], vec![0, 66, 67], vec![65, 66, 0]] {
        let mut bytes = [0; RECOVERY_WIRE_BYTES];
        assert_eq!(
            encode_capture_root_units(key, 1, [9; 24], 3, 0, units.into_iter(), &mut bytes),
            Err(RecoveryWireError::Malformed)
        );
    }
    Ok(())
}
#[test]
fn root_decoder_rejects_odd_offsets_extents_nul_and_zero_physical_identity() -> TestResult {
    let mut bytes = [0; RECOVERY_WIRE_BYTES];
    let (length, _) =
        encode_capture_root_units(key()?, 1, [9; 24], 3, 0, [65, 66, 67].into_iter(), &mut bytes)?;
    for (offset, value) in [(24usize, 5u32), (28, 1), (24, RECOVERY_ROOT_BYTES as u32 + 2), (28, 6)]
    {
        let mut malformed = bytes;
        malformed[RECOVERY_HEADER_BYTES + offset..RECOVERY_HEADER_BYTES + offset + 4]
            .copy_from_slice(&value.to_le_bytes());
        assert!(decode(&malformed[..length], RecoveryReceiver::Supervisor).is_err());
    }
    let mut zero_identity = bytes;
    zero_identity[RECOVERY_HEADER_BYTES..RECOVERY_HEADER_BYTES + 24].fill(0);
    assert_eq!(
        decode(&zero_identity[..length], RecoveryReceiver::Supervisor).err(),
        Some(RecoveryWireError::Malformed)
    );
    let mut nul = bytes;
    nul[RECOVERY_HEADER_BYTES + 34..RECOVERY_HEADER_BYTES + 36].fill(0);
    assert_eq!(
        decode(&nul[..length], RecoveryReceiver::Supervisor).err(),
        Some(RecoveryWireError::Malformed)
    );
    let mut odd = bytes;
    odd[12..16].copy_from_slice(&37u32.to_le_bytes());
    assert_eq!(
        decode(&odd[..length - 1], RecoveryReceiver::Supervisor).err(),
        Some(RecoveryWireError::Malformed)
    );
    Ok(())
}
