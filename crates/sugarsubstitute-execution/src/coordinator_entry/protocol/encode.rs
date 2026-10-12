// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preflight every entry byte before mutating its fixed private frame allocation.
use super::*;
use std::path::Path;
use sugarsubstitute_contracts::{
    configuration::{ConnectionConfiguration, LoopbackHost, Theme},
    coordinator_entry::EntryOwnerDegradation,
};

pub(in crate::coordinator_entry) fn encoded_length(
    header: &Header,
    payload: &Payload<'_>,
) -> Result<usize, EntryTransportError> {
    header.validate(payload.kind())?;
    if let Payload::Accepted(run) = payload
        && (run.owner.namespace != header.namespace || Some(run.owner.epoch) != header.epoch)
    {
        return Err(EntryTransportError::Authentication);
    }
    let body = body_length(payload)?;
    let total = HEADER_BYTES.checked_add(body).ok_or(EntryTransportError::Capacity)?;
    if total > MAX_ENTRY_FRAME_BYTES {
        return Err(EntryTransportError::Capacity);
    }
    Ok(total)
}
fn body_length(payload: &Payload<'_>) -> Result<usize, EntryTransportError> {
    Ok(match payload {
        Payload::Launch(request) => launch_length(request)?,
        Payload::Accepted(_) => 16,
        Payload::Rejected(_) => 2,
        _ => 0,
    })
}

pub(in crate::coordinator_entry) fn encode(
    output: &mut [u8],
    header: &Header,
    payload: Payload<'_>,
) -> Result<usize, EntryTransportError> {
    let length = encoded_length(header, &payload)?;
    if output.len() < length {
        return Err(EntryTransportError::Capacity);
    }
    let mut writer = Writer::new(&mut output[..length]);
    write_header(&mut writer, header, payload.kind(), length - HEADER_BYTES)?;
    match payload {
        Payload::Launch(request) => launch(&mut writer, request)?,
        Payload::Accepted(run) => {
            writer.bytes(&run.instance.get().to_le_bytes())?;
            writer.bytes(&run.generation.get().to_le_bytes())?;
        }
        Payload::Rejected(rejection) => writer.bytes(&rejection_bytes(rejection))?,
        _ => {}
    }
    Ok(length)
}
fn write_header(
    writer: &mut Writer<'_>,
    header: &Header,
    kind: Kind,
    body_length: usize,
) -> Result<(), EntryTransportError> {
    header.validate(kind)?;
    writer.bytes(MAGIC)?;
    writer.u16(VERSION)?;
    writer.u16(kind as u16)?;
    writer.u32(body_length)?;
    writer.u32(0)?;
    writer.bytes(header.namespace.bytes())?;
    writer.bytes(header.epoch.as_ref().map_or(&[0; 16][..], |epoch| epoch.bytes().as_slice()))?;
    writer.bytes(&header.request.get().to_le_bytes())?;
    writer.bytes(&header.source.ticks().to_le_bytes())?;
    writer.bytes(&header.nonce)?;
    writer.bytes(&header.source.frequency().to_le_bytes())?;
    Ok(())
}
pub(in crate::coordinator_entry) fn stage_launch(
    output: &mut [u8],
    request: &EntryLaunchRequest,
) -> Result<usize, EntryTransportError> {
    let length = HEADER_BYTES
        .checked_add(body_length(&Payload::Launch(request))?)
        .ok_or(EntryTransportError::Capacity)?;
    if length > MAX_ENTRY_FRAME_BYTES || output.len() < length {
        return Err(EntryTransportError::Capacity);
    }
    output[..HEADER_BYTES].fill(0);
    let mut writer = Writer::new(&mut output[HEADER_BYTES..length]);
    launch(&mut writer, request)?;
    Ok(length)
}
pub(in crate::coordinator_entry) fn bind_staged_launch(
    output: &mut [u8],
    length: usize,
    header: &Header,
) -> Result<(), EntryTransportError> {
    header.validate(Kind::Launch)?;
    if !(HEADER_BYTES..=MAX_ENTRY_FRAME_BYTES).contains(&length) || output.len() < length {
        return Err(EntryTransportError::Capacity);
    }
    write_header(
        &mut Writer::new(&mut output[..HEADER_BYTES]),
        header,
        Kind::Launch,
        length - HEADER_BYTES,
    )
}

fn add(total: &mut usize, length: usize) -> Result<(), EntryTransportError> {
    *total = total.checked_add(length).ok_or(EntryTransportError::Capacity)?;
    Ok(())
}
fn path_length(total: &mut usize, path: &Path) -> Result<(), EntryTransportError> {
    add(total, 4)?;
    add(total, raw_windows::length(path)?)
}
fn argument_length(arguments: &[String]) -> Result<usize, EntryTransportError> {
    let mut length = 0;
    for argument in arguments {
        add(&mut length, 4)?;
        add(&mut length, argument.len())?;
    }
    Ok(length)
}
fn launch_length(request: &EntryLaunchRequest) -> Result<usize, EntryTransportError> {
    let configuration = request.configuration();
    let presentation = configuration.presentation();
    let locale = presentation.locale().as_str();
    let profile = configuration.profile().as_str();
    u8::try_from(locale.len()).map_err(|_| EntryTransportError::Capacity)?;
    u8::try_from(profile.len()).map_err(|_| EntryTransportError::Capacity)?;
    let mut length = 10;
    add(&mut length, locale.len())?;
    add(&mut length, profile.len())?;
    match configuration.connection() {
        ConnectionConfiguration::Launch(managed) => {
            u8::try_from(managed.arguments().len()).map_err(|_| EntryTransportError::Capacity)?;
            path_length(&mut length, managed.root())?;
            path_length(&mut length, managed.interpreter())?;
            add(&mut length, 4)?;
            add(&mut length, argument_length(managed.arguments())?)?;
        }
        ConnectionConfiguration::Attach(_) => add(&mut length, 1)?,
    }
    for path in [
        request.package_directory(),
        request.metadata_file(),
        request.cube_directory(),
        request.backend_script(),
        request.profile_directory(),
    ] {
        path_length(&mut length, path)?;
    }
    add(&mut length, 4)?;
    add(
        &mut length,
        request.environment().len().checked_mul(2).ok_or(EntryTransportError::Capacity)?,
    )?;
    Ok(length)
}
fn path(writer: &mut Writer<'_>, value: &Path) -> Result<(), EntryTransportError> {
    writer.u32(raw_windows::length(value)?)?;
    for unit in raw_windows::units(value)? {
        writer.u16(unit)?;
    }
    Ok(())
}
fn launch(
    writer: &mut Writer<'_>,
    request: &EntryLaunchRequest,
) -> Result<(), EntryTransportError> {
    let configuration = request.configuration();
    let presentation = configuration.presentation();
    let locale = presentation.locale().as_str();
    let profile = configuration.profile().as_str();
    let (mode, arguments, port) = match configuration.connection() {
        ConnectionConfiguration::Launch(managed) => (1, managed.arguments().len(), managed.port()),
        ConnectionConfiguration::Attach(origin) => (2, 0, origin.port()),
    };
    writer.byte(mode)?;
    writer.byte(match presentation.theme() {
        Theme::Dark => 0,
        Theme::Light => 1,
    })?;
    writer.byte(u8::from(presentation.reduced_motion()))?;
    writer.byte(u8::try_from(locale.len()).map_err(|_| EntryTransportError::Capacity)?)?;
    writer.byte(u8::try_from(profile.len()).map_err(|_| EntryTransportError::Capacity)?)?;
    writer.byte(u8::try_from(arguments).map_err(|_| EntryTransportError::Capacity)?)?;
    writer.u16(port.get())?;
    writer.u16(0)?;
    writer.bytes(locale.as_bytes())?;
    writer.bytes(profile.as_bytes())?;
    match configuration.connection() {
        ConnectionConfiguration::Launch(managed) => {
            path(writer, managed.root())?;
            path(writer, managed.interpreter())?;
            writer.u32(argument_length(managed.arguments())?)?;
            for argument in managed.arguments() {
                writer.u32(argument.len())?;
                writer.bytes(argument.as_bytes())?;
            }
        }
        ConnectionConfiguration::Attach(origin) => writer.byte(match origin.host() {
            LoopbackHost::Ipv4 => 1,
            LoopbackHost::Ipv6 => 2,
            LoopbackHost::Localhost => 3,
        })?,
    }
    for value in [
        request.package_directory(),
        request.metadata_file(),
        request.cube_directory(),
        request.backend_script(),
        request.profile_directory(),
    ] {
        path(writer, value)?;
    }
    writer.u32(request.environment().len() * 2)?;
    for unit in request.environment() {
        writer.u16(*unit)?;
    }
    Ok(())
}
fn rejection_bytes(rejection: EntryRejection) -> [u8; 2] {
    match rejection {
        EntryRejection::InvalidInput => [1, 0],
        EntryRejection::ProfileBusy => [2, 0],
        EntryRejection::Capacity => [3, 0],
        EntryRejection::Deadline => [4, 0],
        EntryRejection::Authentication => [5, 0],
        EntryRejection::ChangedDuplicate => [6, 0],
        EntryRejection::RecoverableContention => [7, 0],
        EntryRejection::StaleRequest => [9, 0],
        EntryRejection::DegradedOwner(reason) => [
            8,
            match reason {
                EntryOwnerDegradation::IdentityUncertain => 1,
                EntryOwnerDegradation::RecordUnknown => 2,
                EntryOwnerDegradation::EndpointCollision => 3,
                EntryOwnerDegradation::NativeCleanupUncertain => 4,
            },
        ],
    }
}
