// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate exact bounded entry sections before exposing an allocation-free request view.
use super::*;
use std::num::NonZeroU16;
use sugarsubstitute_contracts::{
    configuration::{LoopbackOrigin, Presentation, Theme},
    coordinator_entry::{
        EntryArgumentsView, EntryConnectionView, EntryEnvironmentView, EntryFrontendView,
        EntryOwnerDegradation, EntryWindowsPathView,
    },
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
        Kind::Launch => DecodedPayload::Launch(launch(&mut reader)?),
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

fn path<'a>(reader: &mut Reader<'a>) -> Result<EntryWindowsPathView<'a>, EntryTransportError> {
    EntryWindowsPathView::from_le_bytes(reader.section()?).map_err(EntryTransportError::Shape)
}
fn text<'a>(reader: &mut Reader<'a>, length: usize) -> Result<&'a str, EntryTransportError> {
    std::str::from_utf8(reader.bytes(length)?).map_err(|_| EntryTransportError::Malformed)
}
fn launch<'a>(reader: &mut Reader<'a>) -> Result<EntryLaunchRequestView<'a>, EntryTransportError> {
    let mode = reader.byte()?;
    let theme = match reader.byte()? {
        0 => Theme::Dark,
        1 => Theme::Light,
        _ => return Err(EntryTransportError::Malformed),
    };
    let reduced_motion = match reader.byte()? {
        0 => false,
        1 => true,
        _ => return Err(EntryTransportError::Malformed),
    };
    let locale_length = reader.byte()? as usize;
    let profile_length = reader.byte()? as usize;
    let argument_count = reader.byte()?;
    let port = NonZeroU16::new(reader.u16()?).ok_or(EntryTransportError::Malformed)?;
    if reader.u16()? != 0 {
        return Err(EntryTransportError::Malformed);
    }
    let locale = text(reader, locale_length)?;
    let profile = text(reader, profile_length)?;
    let presentation = Presentation::new(locale, theme, reduced_motion).map_err(|cause| {
        EntryTransportError::Shape(
            sugarsubstitute_contracts::coordinator_entry::EntryShapeError::Configuration(cause),
        )
    })?;
    let connection = match mode {
        1 => EntryConnectionView::Managed {
            root: path(reader)?,
            interpreter: path(reader)?,
            arguments: EntryArgumentsView::from_encoded(reader.section()?, argument_count)
                .map_err(EntryTransportError::Shape)?,
            port,
        },
        2 if argument_count == 0 => {
            EntryConnectionView::Attached(origin(reader.byte()?, port.get())?)
        }
        _ => return Err(EntryTransportError::Malformed),
    };
    let frontend =
        EntryFrontendView::new(path(reader)?, path(reader)?, path(reader)?, path(reader)?);
    let profile_directory = path(reader)?;
    let environment = EntryEnvironmentView::from_le_bytes(reader.section()?)
        .map_err(EntryTransportError::Shape)?;
    EntryLaunchRequestView::new(
        connection,
        frontend,
        profile_directory,
        profile,
        presentation,
        environment,
    )
    .map_err(EntryTransportError::Shape)
}

fn origin(host: u8, port: u16) -> Result<LoopbackOrigin, EntryTransportError> {
    let prefix: &[u8] = match host {
        1 => b"http://127.0.0.1:",
        2 => b"http://[::1]:",
        3 => b"http://localhost:",
        _ => return Err(EntryTransportError::Malformed),
    };
    let mut bytes = [0; 32];
    bytes[..prefix.len()].copy_from_slice(prefix);
    let mut digits = [0; 5];
    let mut count = 0;
    let mut remaining = port;
    loop {
        digits[count] = b'0' + (remaining % 10) as u8;
        count += 1;
        remaining /= 10;
        if remaining == 0 {
            break;
        }
    }
    for index in 0..count {
        bytes[prefix.len() + index] = digits[count - index - 1];
    }
    let url = std::str::from_utf8(&bytes[..prefix.len() + count])
        .map_err(|_| EntryTransportError::Malformed)?;
    LoopbackOrigin::parse(url, port).map_err(|cause| {
        EntryTransportError::Shape(
            sugarsubstitute_contracts::coordinator_entry::EntryShapeError::Configuration(cause),
        )
    })
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
