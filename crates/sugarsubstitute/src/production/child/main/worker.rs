// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Construct native request data centrally from the identical guarded input and grant frame.
use super::*;
use crate::production_output::OutputBinding;
use std::{collections::TryReserveError, fmt, path::PathBuf};
use sugarsubstitute_contracts::configuration::ProfileId;
use sugarsubstitute_desktop::{MainRequestError, MainWindowRequest, ServedFrontendVersions};
use sugarsubstitute_supervision::lifecycle_protocol::{
    DecodedPayload, FrameCodec, ProtocolError, ReplyOutcome,
};
pub(super) struct PreparedMain {
    pub(super) profile: ProfileTransaction,
    pub(super) frame: Option<ReceivedLifecycleFrame>,
    pub(super) header: WireHeader,
    pub(super) original: Moment,
    pub(super) binding: OutputBinding,
    _input: PreparedMainInput,
    _control: MemoryClaim,
}
#[derive(Debug)]
pub(super) enum MainError {
    State,
    Cancelled,
    Deadline,
    Protocol(ProtocolError),
    Request(MainRequestError),
    Profile(profile_transaction::ProfileError),
    Configuration(sugarsubstitute_contracts::configuration::ConfigurationError),
    Allocation(TryReserveError),
    Memory(MemoryClaimError),
}
impl fmt::Display for MainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Cancelled => "Original main request was cancelled",
            Self::Deadline => "Original main request deadline expired",
            _ => "Actual main request could not be prepared",
        })
    }
}
impl Error for MainError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Protocol(e) => Some(e),
            Self::Request(e) => Some(e),
            Self::Profile(e) => Some(e),
            Self::Configuration(e) => Some(e),
            Self::Allocation(e) => Some(e),
            Self::Memory(e) => Some(e),
            Self::State | Self::Cancelled | Self::Deadline => None,
        }
    }
}
pub(super) struct MainConstruction {
    pub(super) frame: Option<ReceivedLifecycleFrame>,
    pub(super) input: PreparedMainInput,
    pub(super) identity: MainIdentity,
    pub(super) header: WireHeader,
    pub(super) origin: sugarsubstitute_contracts::configuration::LoopbackOrigin,
    pub(super) settings: MainWindowSettings,
    pub(super) original: Moment,
}
pub(super) fn submit(
    client: &ExecutionClient,
    scope: &Scope,
    id: JobId,
    input: &mut Option<MainConstruction>,
) -> std::result::Result<MainJob, sugarsubstitute_execution::BlockingInputError> {
    let original =
        input.as_ref().ok_or(sugarsubstitute_execution::BlockingInputError::MissingInput)?.original;
    client.submit_blocking_owned_input(
        scope,
        JobSpec {
            id,
            operation: Operation::Startup,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(original),
            reservation_bytes: MainWindowRequest::KNOWN_STORAGE_BYTES,
            replacement: None,
        },
        input,
        move |construction, context| {
            let MainConstruction { frame, input, identity, header, origin, settings, original } =
                construction;
            let result = (|| {
                check(&context, original)?;
                let control = context
                    .memory()
                    .claim((size_of::<PreparedMain>() + size_of::<ProfileTransaction>()) as u64)
                    .map_err(MainError::Memory)?;
                let decoded =
                    FrameCodec::decode(frame.as_ref().ok_or(MainError::State)?.as_bytes())
                        .map_err(MainError::Protocol)?;
                if decoded.header != header {
                    return Err(MainError::State);
                }
                let DecodedPayload::Reply(ReplyOutcome::Granted(grant)) = decoded.payload else {
                    return Err(MainError::State);
                };
                if grant.identity != identity
                    || grant.origin != origin
                    || grant.lease.resource.get() == 0
                {
                    return Err(MainError::State);
                }
                let temporary = context.memory().claim(192 * 1024).map_err(MainError::Memory)?;
                let mut directory = PathBuf::new();
                directory
                    .try_reserve_exact(input.directory().as_os_str().len())
                    .map_err(MainError::Allocation)?;
                directory.push(input.directory());
                let profile = ProfileId::new(copy(input.profile().as_str())?)
                    .map_err(MainError::Configuration)?;
                let versions = ServedFrontendVersions::new(
                    copy(grant.frontend.as_str())?,
                    copy(grant.sugarcubes.as_str())?,
                )
                .map_err(MainError::Request)?;
                let request = Box::new(
                    MainWindowRequest::new(
                        identity,
                        origin,
                        profile,
                        directory,
                        versions,
                        settings,
                        context.memory().clone(),
                    )
                    .map_err(MainError::Request)?,
                );
                drop(temporary);
                check(&context, original)?;
                let profile = ProfileTransaction::new(request, original, &context)
                    .map_err(MainError::Profile)?;
                check(&context, original)?;
                let binding = OutputBinding {
                    resource: ResourceKey {
                        owner: identity.run.owner,
                        id: grant.lease.resource,
                        generation: grant.lease.generation,
                    },
                    run: identity.run,
                    connection: header.connection.ok_or(MainError::State)?,
                    lease: grant.lease,
                };
                Ok::<_, MainError>((profile, binding, control))
            })();
            match result {
                Ok((profile, binding, control)) => Ok(PreparedMain {
                    profile,
                    frame,
                    header,
                    original,
                    binding,
                    _input: input,
                    _control: control,
                }),
                Err(cause) => Err(MainPreparationFailure::new(cause, frame, input)),
            }
        },
    )
}
fn check(context: &JobContext, original: Moment) -> std::result::Result<(), MainError> {
    if context.cancellation_requested() {
        return Err(MainError::Cancelled);
    }
    let until = context.deadline().filter(|until| *until <= original).ok_or(MainError::State)?;
    if context.now() >= until {
        return Err(MainError::Deadline);
    }
    if context.cancellation_requested() {
        return Err(MainError::Cancelled);
    }
    Ok(())
}
fn copy(value: &str) -> std::result::Result<String, MainError> {
    let mut owned = String::new();
    owned.try_reserve_exact(value.len()).map_err(MainError::Allocation)?;
    owned.push_str(value);
    Ok(owned)
}
