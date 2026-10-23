// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode complete fixed-schema frames without allocating launch data.
use super::{
    ArgumentRefs, CapabilityRef, DecodedFrame, DecodedPayload, DecodedTarget, FolderFragment,
    HEADER_BYTES, LeaseReference, MAX_FRAME_BYTES, ManagedTargetRef, ProtocolError, Utf16Text,
    WIRE_VERSION, WireHeader, binary::Reader, main_fields, reply_codec,
};
use std::num::NonZeroU16;
use sugarsubstitute_contracts::{
    configuration::{ConfigurationError, LoopbackHost},
    lifecycle::{
        CompletionIntent, ConnectionId, CoordinatorEpoch, CoordinatorKey, InstanceId, LeaseId,
        NamespaceId, RequestId, ResourceGeneration, ResourceId, RunGeneration, RunKey, ShutdownId,
        UiChallengeId, UiWatchdogActivity,
    },
};

pub(super) fn frame(source: &[u8]) -> Result<DecodedFrame<'_>, ProtocolError> {
    if source.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    if source.len() < HEADER_BYTES {
        return Err(ProtocolError::Truncated);
    }
    let mut input = Reader::new(source);
    if input.take(4)? != b"SSLC"
        || input.u16()? != WIRE_VERSION
        || usize::from(input.u16()?) != HEADER_BYTES
    {
        return Err(ProtocolError::Schema);
    }
    if usize::try_from(input.u32()?).map_err(|_| ProtocolError::FrameLength)? != source.len() {
        return Err(ProtocolError::FrameLength);
    }
    let kind = input.u16()?;
    if input.u16()? != 0 {
        return Err(ProtocolError::UnsupportedTag);
    }
    let owner = CoordinatorKey {
        namespace: NamespaceId::new(*input.array()?),
        epoch: CoordinatorEpoch::new(*input.array()?).map_err(|_| ProtocolError::Identity)?,
    };
    let run = RunKey {
        owner,
        instance: InstanceId::new(input.u64()?).map_err(|_| ProtocolError::Identity)?,
        generation: RunGeneration::new(input.u64()?).map_err(|_| ProtocolError::Identity)?,
    };
    let connection = input.u64()?;
    let request = input.u64()?;
    let header = WireHeader {
        run,
        connection: if connection == 0 {
            None
        } else {
            Some(ConnectionId::new(connection).map_err(|_| ProtocolError::Identity)?)
        },
        request: if request == 0 {
            None
        } else {
            Some(RequestId::new(request).map_err(|_| ProtocolError::Identity)?)
        },
        deadline_qpc: input.i64()?,
    };
    header.validate(kind)?;
    let payload = match kind {
        1 => DecodedPayload::Hello(CapabilityRef::new(input.array()?)),
        2 => {
            let qpc_frequency = input.i64()?;
            if qpc_frequency <= 0 {
                return Err(ProtocolError::Clock);
            }
            DecodedPayload::Welcome { qpc_frequency }
        }
        3 => DecodedPayload::Register,
        4 => DecodedPayload::Acquire {
            identity: main_fields::read_identity(&mut input, run)?,
            target: target(&mut input, source)?,
        },
        5 => DecodedPayload::Release(lease(&mut input)?),
        6 => DecodedPayload::Reload {
            lease: lease(&mut input)?,
            identity: main_fields::read_identity(&mut input, run)?,
        },
        7 => DecodedPayload::ExitIntent {
            shutdown: ShutdownId::new(input.u64()?).map_err(|_| ProtocolError::Identity)?,
            intent: intent(input.u8()?)?,
            expected_code: input.u32()?,
        },
        8 => DecodedPayload::ExitCompleted(
            ShutdownId::new(input.u64()?).map_err(|_| ProtocolError::Identity)?,
        ),
        9 => DecodedPayload::UiChallenge(
            UiChallengeId::new(input.u64()?).map_err(|_| ProtocolError::Identity)?,
        ),
        10 => DecodedPayload::UiResponse(
            UiChallengeId::new(input.u64()?).map_err(|_| ProtocolError::Identity)?,
        ),
        11 => DecodedPayload::Reply(reply_codec::read(&mut input, run)?),
        12 => DecodedPayload::RequestedProfileFolder(FolderFragment::read(&mut input, run)?),
        13 => DecodedPayload::EnvironmentPrepared(FolderFragment::read(&mut input, run)?),
        14 => DecodedPayload::ControllerPrepared {
            identity: main_fields::read_identity(&mut input, run)?,
            browser_process_id: main_fields::read_pid(&mut input)?,
        },
        15 => DecodedPayload::MainRetired(main_fields::read_identity(&mut input, run)?),
        16 => DecodedPayload::NativeActivity {
            revision: main_fields::read_positive(&mut input)?,
            activity: match input.u8()? {
                1 => UiWatchdogActivity::VisibleActive,
                2 => UiWatchdogActivity::Inactive,
                3 => UiWatchdogActivity::Suspended,
                _ => return Err(ProtocolError::UnsupportedTag),
            },
        },
        17 => DecodedPayload::OutputPull {
            lease: lease(&mut input)?,
            acknowledged_delivery: std::num::NonZeroU64::new(input.u64()?),
        },
        18 => DecodedPayload::MainFailed(main_fields::read_stage_failure(&mut input, run)?),
        19 => DecodedPayload::SettleAttempt {
            identity: main_fields::read_identity(&mut input, run)?,
            acquisition: RequestId::new(input.u64()?).map_err(|_| ProtocolError::Identity)?,
        },
        21 => DecodedPayload::MainRevealed(main_fields::read_identity(&mut input, run)?),
        20 => DecodedPayload::RetryBackend {
            identity: main_fields::read_identity(&mut input, run)?,
            failed: lease(&mut input)?,
        },
        _ => return Err(ProtocolError::UnsupportedTag),
    };
    input.finish()?;
    Ok(DecodedFrame { header, payload })
}
fn target<'a>(
    input: &mut Reader<'a>,
    source: &'a [u8],
) -> Result<DecodedTarget<'a>, ProtocolError> {
    match input.u8()? {
        1 => {
            let port = port(input.u16()?)?;
            let count = usize::from(input.u8()?);
            if count > sugarsubstitute_contracts::configuration::MAX_ARGUMENTS {
                return Err(ProtocolError::Length);
            }
            let root = Utf16Text::read(input, 32766)?;
            let interpreter = Utf16Text::read(input, 32766)?;
            let arguments = ArgumentRefs::read(input, count, source)?;
            Ok(DecodedTarget::Managed(ManagedTargetRef { root, interpreter, arguments, port }))
        }
        2 => {
            let host = match input.u8()? {
                1 => LoopbackHost::Ipv4,
                2 => LoopbackHost::Ipv6,
                3 => LoopbackHost::Localhost,
                _ => return Err(ProtocolError::UnsupportedTag),
            };
            Ok(DecodedTarget::Attached { host, port: port(input.u16()?)? })
        }
        _ => Err(ProtocolError::UnsupportedTag),
    }
}
fn port(value: u16) -> Result<NonZeroU16, ProtocolError> {
    NonZeroU16::new(value).ok_or(ProtocolError::Configuration(ConfigurationError::InvalidPort))
}
pub(super) fn lease(input: &mut Reader<'_>) -> Result<LeaseReference, ProtocolError> {
    Ok(LeaseReference {
        resource: ResourceId::new(input.u64()?).map_err(|_| ProtocolError::Identity)?,
        generation: ResourceGeneration::new(input.u64()?).map_err(|_| ProtocolError::Identity)?,
        lease: LeaseId::new(input.u64()?).map_err(|_| ProtocolError::Identity)?,
    })
}
fn intent(tag: u8) -> Result<CompletionIntent, ProtocolError> {
    match tag {
        1 => Ok(CompletionIntent::Normal),
        2 => Ok(CompletionIntent::StartupCancelled),
        3 => Ok(CompletionIntent::StartupFailed),
        _ => Err(ProtocolError::UnsupportedTag),
    }
}
