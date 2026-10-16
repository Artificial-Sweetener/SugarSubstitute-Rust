// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the closed command bodies, bounds and exact symmetric native payload grammar.
use super::fields::*;
use super::*;
use crate::incident::{IncidentRevision, IncidentUtc};
use key::{array, number, put};
pub(super) fn write(command: &RecoveryCommand<'_>, body: &mut [u8]) {
    match command {
        RecoveryCommand::Hello(capability) => body.copy_from_slice(capability),
        RecoveryCommand::Startup { total, offset, bytes } => {
            write32(body, 0, *total);
            write32(body, 4, *offset);
            body[8..].copy_from_slice(bytes);
        }
        RecoveryCommand::CaptureRoot { identity, total, offset, units_le } => {
            body[..24].copy_from_slice(identity);
            write32(body, 24, *total);
            write32(body, 28, *offset);
            body[32..].copy_from_slice(units_le);
        }
        RecoveryCommand::DesktopIdentity { launch, navigation } => {
            put(body, 0, launch.get());
            put(body, 8, navigation.get());
        }
        RecoveryCommand::Commit(ack) => {
            put(body, 0, ack.revision().get());
            put(body, 8, ack.at().unix_seconds());
        }
        RecoveryCommand::CaptureRegistered { supervisor, handler } => {
            process(body, *supervisor);
            process(&mut body[16..], *handler);
        }
        RecoveryCommand::DesktopCreated(value) | RecoveryCommand::DesktopEnrolled(value) => {
            process(body, *value)
        }
        RecoveryCommand::DesktopExited { process: value, status } => {
            process(body, *value);
            write32(body, 16, *status);
        }
        RecoveryCommand::Stop(until) => {
            counter(body, *until);
        }
        RecoveryCommand::Failed(code) => write32(body, 0, failure(*code)),
        RecoveryCommand::StartupAccepted | RecoveryCommand::Retired => {}
    }
}
pub(super) fn read(
    kind: u32,
    key: RecoveryGrantKey,
    body: &[u8],
) -> Result<RecoveryCommand<'_>, RecoveryWireError> {
    let length = body.len();
    Ok(match kind {
        1 if length == 32 => {
            let capability = array(body, 0)?;
            if capability == [0; 32] {
                return Err(RecoveryWireError::Malformed);
            }
            RecoveryCommand::Hello(capability)
        }
        2 if length > 8 => {
            let total = read32(body, 0)?;
            let offset = read32(body, 4)?;
            let bytes = &body[8..];
            chunk(total, offset, bytes.len(), RECOVERY_STARTUP_BYTES, RECOVERY_STARTUP_CHUNK)?;
            RecoveryCommand::Startup { total, offset, bytes }
        }
        3 if length == 16 => {
            let revision =
                IncidentRevision::new(number(body, 0)?).map_err(|_| RecoveryWireError::Binding)?;
            if key.report().revision().next().map_err(|_| RecoveryWireError::Binding)? != revision {
                return Err(RecoveryWireError::Binding);
            }
            let utc = i64::try_from(number(body, 8)?).map_err(|_| RecoveryWireError::Malformed)?;
            let at =
                IncidentUtc::from_unix_seconds(utc).map_err(|_| RecoveryWireError::Malformed)?;
            RecoveryCommand::Commit(DurableAcknowledgement::new(
                key.report().incident(),
                revision,
                at,
            ))
        }
        4 if length == 0 => RecoveryCommand::StartupAccepted,
        5 if length == 32 => {
            let supervisor = read_process(body)?;
            let handler = read_process(&body[16..])?;
            if supervisor.pid() == handler.pid() {
                return Err(RecoveryWireError::Binding);
            }
            RecoveryCommand::CaptureRegistered { supervisor, handler }
        }
        6 if length == 16 => RecoveryCommand::DesktopCreated(read_process(body)?),
        8 if length == 24 && body[20..24] == [0; 4] => RecoveryCommand::DesktopExited {
            process: read_process(body)?,
            status: read32(body, 16)?,
        },
        11 if length == 16 => RecoveryCommand::Stop(read_counter(body)?),
        12 if length == 0 => RecoveryCommand::Retired,
        13 if length == 4 => RecoveryCommand::Failed(read_failure(read32(body, 0)?)?),
        14 if length > 32 => {
            let identity = array(body, 0)?;
            let total = read32(body, 24)?;
            let offset = read32(body, 28)?;
            let units_le = &body[32..];
            root_chunk(identity, total, offset, units_le)?;
            RecoveryCommand::CaptureRoot { identity, total, offset, units_le }
        }
        16 if length == 16 => RecoveryCommand::DesktopEnrolled(read_process(body)?),
        17 if length == 16 => RecoveryCommand::DesktopIdentity {
            launch: crate::startup::LaunchId::new(number(body, 0)?)
                .map_err(|_| RecoveryWireError::Binding)?,
            navigation: crate::execution::Generation::new(number(body, 8)?)
                .map_err(|_| RecoveryWireError::Binding)?,
        },
        _ => return Err(RecoveryWireError::Malformed),
    })
}
pub(super) fn shape(command: &RecoveryCommand<'_>) -> Result<(u32, usize), RecoveryWireError> {
    Ok(match command {
        RecoveryCommand::Hello(capability) if *capability != [0; 32] => (1, 32),
        RecoveryCommand::Hello(_) => return Err(RecoveryWireError::Malformed),
        RecoveryCommand::Startup { total, offset, bytes } => {
            chunk(*total, *offset, bytes.len(), RECOVERY_STARTUP_BYTES, RECOVERY_STARTUP_CHUNK)?;
            (2, 8 + bytes.len())
        }

        RecoveryCommand::CaptureRoot { identity, total, offset, units_le } => {
            root_chunk(*identity, *total, *offset, units_le)?;
            (14, 32 + units_le.len())
        }
        RecoveryCommand::DesktopIdentity { .. } => (17, 16),
        RecoveryCommand::Commit(_) => (3, 16),
        RecoveryCommand::StartupAccepted => (4, 0),
        RecoveryCommand::CaptureRegistered { supervisor, handler } => {
            if supervisor.pid() == handler.pid() {
                return Err(RecoveryWireError::Binding);
            }
            (5, 32)
        }
        RecoveryCommand::DesktopCreated(_) => (6, 16),
        RecoveryCommand::DesktopEnrolled(_) => (16, 16),
        RecoveryCommand::DesktopExited { .. } => (8, 24),
        RecoveryCommand::Stop(_) => (11, 16),
        RecoveryCommand::Retired => (12, 0),
        RecoveryCommand::Failed(_) => (13, 4),
    })
}
