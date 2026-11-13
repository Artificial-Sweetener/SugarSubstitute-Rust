// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate exact bounded entry sections before exposing an allocation-free request view.
use super::*;
use sugarsubstitute_contracts::{
    coordinator_entry::EntryOwnerDegradation,
    lifecycle::{CoordinatorKey, InstanceId, RunGeneration},
};

pub(in crate::coordinator_entry) fn decode(bytes: &[u8]) -> Result<Frame<'_>, EntryTransportError> {
    if !(HEADER_BYTES..=MAX_ENTRY_FRAME_BYTES).contains(&bytes.len()) {
        return Err(EntryTransportError::Malformed);
    }
    let mut reader = Reader::new(bytes);
    if reader.bytes(4)? != MAGIC || reader.u16()? != VERSION {
        return Err(EntryTransportError::Malformed);
    }
    let kind = Kind::from_raw(reader.u16()?)?;
    if reader.u32()? != bytes.len() - HEADER_BYTES || reader.u32()? != 0 {
        return Err(EntryTransportError::Malformed);
    }
    let namespace = NamespaceId::new(reader.array()?);
    let epoch_bytes = reader.array::<16>()?;
    let epoch = if epoch_bytes.iter().all(|byte| *byte == 0) {
        None
    } else {
        Some(CoordinatorEpoch::new(epoch_bytes).map_err(|_| EntryTransportError::Malformed)?)
    };
    let request = EntryRequestId::new(u64::from_le_bytes(reader.array()?))
        .map_err(|_| EntryTransportError::Malformed)?;
    let ticks = i64::from_le_bytes(reader.array()?);
    let nonce = reader.array()?;
    let frequency = i64::from_le_bytes(reader.array()?);
    let source =
        CounterDeadline::new(ticks, frequency).map_err(|_| EntryTransportError::Malformed)?;
    let header = Header { namespace, epoch, request, source, nonce };
    header.validate(kind)?;
    let payload = match kind {
        Kind::Hello => DecodedPayload::Hello,
        Kind::Challenge => DecodedPayload::Challenge,
        Kind::Initializing => DecodedPayload::Initializing,
        Kind::Closing => DecodedPayload::Closing,
        Kind::Unresponsive => DecodedPayload::Unresponsive,
        Kind::Launch => DecodedPayload::Launch(launch_payload::decode(
            reader.bytes(bytes.len() - HEADER_BYTES)?,
        )?),
        Kind::Accepted => DecodedPayload::Accepted(RunKey {
            owner: CoordinatorKey {
                namespace,
                epoch: epoch.ok_or(EntryTransportError::Malformed)?,
            },
            instance: InstanceId::new(u64::from_le_bytes(reader.array()?))
                .map_err(|_| EntryTransportError::Malformed)?,
            generation: RunGeneration::new(u64::from_le_bytes(reader.array()?))
                .map_err(|_| EntryTransportError::Malformed)?,
        }),
        Kind::Rejected => DecodedPayload::Rejected(rejection(reader.array()?)?),
    };
    if !reader.finished() {
        return Err(EntryTransportError::Malformed);
    }
    Ok(Frame { header, payload })
}

fn rejection(bytes: [u8; 2]) -> Result<EntryRejection, EntryTransportError> {
    match bytes {
        [1, 0] => Ok(EntryRejection::InvalidInput),
        [2, 0] => Ok(EntryRejection::ProfileBusy),
        [3, 0] => Ok(EntryRejection::Capacity),
        [4, 0] => Ok(EntryRejection::Deadline),
        [5, 0] => Ok(EntryRejection::Authentication),
        [6, 0] => Ok(EntryRejection::ChangedDuplicate),
        [7, 0] => Ok(EntryRejection::RecoverableContention),
        [9, 0] => Ok(EntryRejection::StaleRequest),
        [8, reason] => Ok(EntryRejection::DegradedOwner(match reason {
            1 => EntryOwnerDegradation::IdentityUncertain,
            2 => EntryOwnerDegradation::RecordUnknown,
            3 => EntryOwnerDegradation::EndpointCollision,
            4 => EntryOwnerDegradation::NativeCleanupUncertain,
            _ => return Err(EntryTransportError::Malformed),
        })),
        _ => Err(EntryTransportError::Malformed),
    }
}
