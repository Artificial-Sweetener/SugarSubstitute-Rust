// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode exact main scalar fields and loopback origins without allocating native authority.
use super::{
    MainFailureCode, MainStage, MainStageFailure, ProtocolError,
    binary::{Reader, Writer},
};
use std::num::{NonZeroU32, NonZeroU64};
use sugarsubstitute_contracts::{
    configuration::{LoopbackHost, LoopbackOrigin},
    execution::Generation,
    lifecycle::RunKey,
    startup::{LaunchId, MainIdentity},
};

pub(super) fn read_identity(
    input: &mut Reader<'_>,
    run: RunKey,
) -> Result<MainIdentity, ProtocolError> {
    Ok(MainIdentity {
        run,
        launch: LaunchId::new(input.u64()?).map_err(|_| ProtocolError::Identity)?,
        navigation: Generation::new(input.u64()?).map_err(|_| ProtocolError::Identity)?,
    })
}
pub(super) fn write_identity(
    identity: MainIdentity,
    run: RunKey,
    writer: &mut Writer<'_>,
) -> Result<(), ProtocolError> {
    if identity.run != run {
        return Err(ProtocolError::Identity);
    }
    writer.u64(identity.launch.get())?;
    writer.u64(identity.navigation.get())
}
pub(super) fn read_pid(input: &mut Reader<'_>) -> Result<NonZeroU32, ProtocolError> {
    NonZeroU32::new(input.u32()?).ok_or(ProtocolError::Identity)
}
pub(super) fn read_positive(input: &mut Reader<'_>) -> Result<NonZeroU64, ProtocolError> {
    NonZeroU64::new(input.u64()?).ok_or(ProtocolError::Identity)
}
pub(super) fn write_origin(
    origin: LoopbackOrigin,
    writer: &mut Writer<'_>,
) -> Result<(), ProtocolError> {
    writer.u8(match origin.host() {
        LoopbackHost::Ipv4 => 1,
        LoopbackHost::Ipv6 => 2,
        LoopbackHost::Localhost => 3,
    })?;
    writer.u16(origin.port().get())
}
pub(super) fn read_origin(input: &mut Reader<'_>) -> Result<LoopbackOrigin, ProtocolError> {
    let prefix: &[u8] = match input.u8()? {
        1 => b"http://127.0.0.1:",
        2 => b"http://[::1]:",
        3 => b"http://localhost:",
        _ => return Err(ProtocolError::UnsupportedTag),
    };
    let port = input.u16()?;
    if port == 0 {
        return Err(ProtocolError::Configuration(
            sugarsubstitute_contracts::configuration::ConfigurationError::InvalidPort,
        ));
    }
    // A u16 port needs at most five ASCII digits; the exact origin fits this stack buffer.
    let mut digits = [0u8; 5];
    let mut start = digits.len();
    let mut remainder = port;
    while remainder > 0 {
        start -= 1;
        digits[start] = b'0' + (remainder % 10) as u8;
        remainder /= 10;
    }
    let mut bytes = [0u8; 32];
    bytes[..prefix.len()].copy_from_slice(prefix);
    let length = prefix.len() + digits.len() - start;
    bytes[prefix.len()..length].copy_from_slice(&digits[start..]);
    let text = std::str::from_utf8(&bytes[..length]).map_err(|_| ProtocolError::Length)?;
    LoopbackOrigin::parse(text, port).map_err(ProtocolError::Configuration)
}
pub(super) fn read_stage_failure(
    input: &mut Reader<'_>,
    run: RunKey,
) -> Result<MainStageFailure, ProtocolError> {
    let identity = read_identity(input, run)?;
    let stage = match input.u8()? {
        1 => MainStage::Environment,
        2 => MainStage::Browser,
        3 => MainStage::Navigation,
        4 => MainStage::Frontend,
        5 => MainStage::NativeRetirement,
        _ => return Err(ProtocolError::UnsupportedTag),
    };
    let failure = match input.u8()? {
        1 => MainFailureCode::ValidationFailed,
        2 => MainFailureCode::IdentityMismatch,
        3 => MainFailureCode::OutsideOwnedFamily,
        4 => MainFailureCode::Unavailable,
        5 => MainFailureCode::Deadline,
        6 => MainFailureCode::NativeCleanupUncertain,
        _ => return Err(ProtocolError::UnsupportedTag),
    };
    Ok(MainStageFailure { identity, stage, failure })
}
pub(super) fn write_stage_failure(
    value: MainStageFailure,
    run: RunKey,
    writer: &mut Writer<'_>,
) -> Result<(), ProtocolError> {
    write_identity(value.identity, run, writer)?;
    writer.u8(match value.stage {
        MainStage::Environment => 1,
        MainStage::Browser => 2,
        MainStage::Navigation => 3,
        MainStage::Frontend => 4,
        MainStage::NativeRetirement => 5,
    })?;
    writer.u8(match value.failure {
        MainFailureCode::ValidationFailed => 1,
        MainFailureCode::IdentityMismatch => 2,
        MainFailureCode::OutsideOwnedFamily => 3,
        MainFailureCode::Unavailable => 4,
        MainFailureCode::Deadline => 5,
        MainFailureCode::NativeCleanupUncertain => 6,
    })
}
