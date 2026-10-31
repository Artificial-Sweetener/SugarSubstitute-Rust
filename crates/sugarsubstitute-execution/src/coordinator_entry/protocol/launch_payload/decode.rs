// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate complete immutable Launch bodies with the original domain shape owners.
use super::*;
use std::num::NonZeroU16;
use sugarsubstitute_contracts::{
    configuration::{LoopbackOrigin, Presentation, Theme},
    coordinator_entry::{
        EntryArgumentsView, EntryCanvasRendererView, EntryConnectionView, EntryEnvironmentView,
        EntryFrontendView, EntryWindowsPathView,
    },
};

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
    let renderer_kind = reader.byte()?;
    let renderer_length = reader.byte()? as usize;
    let canvas_renderer = match (renderer_kind, renderer_length) {
        (0, 0) => EntryCanvasRendererView::Absent,
        (1, length) => EntryCanvasRendererView::Stored(text(reader, length)?),
        (2, 0) => EntryCanvasRendererView::Malformed,
        _ => return Err(EntryTransportError::Malformed),
    };
    EntryLaunchRequestView::new(
        connection,
        frontend,
        profile_directory,
        profile,
        presentation,
        environment,
    )
    .and_then(|view| view.with_canvas_renderer(canvas_renderer))
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

/// Consume the whole bounded body without allocation or authentication authority.
/// Preserves exact domain validation causes and rejects trailing bytes.
pub(crate) fn decode(body: &[u8]) -> Result<EntryLaunchRequestView<'_>, EntryTransportError> {
    if body.is_empty() || body.len() > MAX_BODY_BYTES {
        return Err(EntryTransportError::Malformed);
    }
    let mut reader = Reader::new(body);
    let view = launch(&mut reader)?;
    if !reader.finished() {
        return Err(EntryTransportError::Malformed);
    }
    Ok(view)
}
