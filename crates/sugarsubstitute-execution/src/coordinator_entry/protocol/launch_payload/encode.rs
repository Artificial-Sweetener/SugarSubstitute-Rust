// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preflight and encode original Launch body bytes without a second frame grammar.
use super::*;
use std::path::Path;
use sugarsubstitute_contracts::configuration::{
    CanvasRendererPreference, ConnectionConfiguration, LoopbackHost, MAX_CANVAS_RENDERER_BYTES,
    Theme,
};

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
/// Bound the unchanged complete Launch body before encoding any output.
pub(crate) fn encoded_length(request: &EntryLaunchRequest) -> Result<usize, EntryTransportError> {
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
    add(&mut length, 2)?;
    if let CanvasRendererPreference::Stored(text) = configuration.canvas_renderer() {
        if text.len() > MAX_CANVAS_RENDERER_BYTES {
            return Err(EntryTransportError::Capacity);
        }
        add(&mut length, text.len())?;
    }
    add(&mut length, if request.originals().is_some() { 33 } else { 1 })?;
    if length > MAX_BODY_BYTES {
        return Err(EntryTransportError::Capacity);
    }
    Ok(length)
}
pub(super) fn path(writer: &mut Writer<'_>, value: &Path) -> Result<(), EntryTransportError> {
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
    match configuration.canvas_renderer() {
        CanvasRendererPreference::Absent => {
            writer.byte(0)?;
            writer.byte(0)?;
        }
        CanvasRendererPreference::Stored(text) => {
            writer.byte(1)?;
            writer.byte(u8::try_from(text.len()).map_err(|_| EntryTransportError::Capacity)?)?;
            writer.bytes(text.as_bytes())?;
        }
        CanvasRendererPreference::Malformed => {
            writer.byte(2)?;
            writer.byte(0)?;
        }
    }
    match request.originals() {
        None => writer.byte(0)?,
        Some(reference) => {
            writer.byte(1)?;
            writer.bytes(reference.manifest_sha256())?;
        }
    }
    Ok(())
}

/// Encode into caller-owned charged storage after whole-body preflight.
/// Rejects insufficient capacity before touching the output.
pub(crate) fn encode(
    output: &mut [u8],
    request: &EntryLaunchRequest,
) -> Result<usize, EntryTransportError> {
    let length = encoded_length(request)?;
    let bytes = output.get_mut(..length).ok_or(EntryTransportError::Capacity)?;
    launch(&mut Writer::new(bytes), request)?;
    Ok(length)
}
