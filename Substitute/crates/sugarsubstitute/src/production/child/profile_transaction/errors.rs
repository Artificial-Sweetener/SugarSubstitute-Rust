// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain genuine allocation/protocol/channel causes and original rejected observations.
use super::*;
use std::{collections::TryReserveError, error::Error, fmt};
use sugarsubstitute_contracts::{
    configuration::ConfigurationError,
    execution::MemoryClaimError,
    lifecycle::{LifecycleFailureCode, LifecycleRejection},
};
use sugarsubstitute_supervision::lifecycle_protocol::{MainStageFailure, ProtocolError};
/// Reject a stage without exposing profile paths or native frame bytes.
#[derive(Debug)]
pub(in super::super) enum ProfileError {
    Closed,
    Cancelled,
    Deadline,
    Folder,
    Identity,
    Order,
    Reply,
    ChannelClosed,
    CoordinatorFailed(LifecycleFailureCode),
    CoordinatorRejected(LifecycleRejection),
    Configuration(ConfigurationError),
    Request(sugarsubstitute_desktop::MainRequestError),
    Stage(MainStageFailure),
    Claim(MemoryClaimError),
    Allocation(TryReserveError),
    Protocol(ProtocolError),
}
impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoordinatorFailed(code) => write!(f, "Coordinator failed: {code:?}"),
            Self::CoordinatorRejected(reason) => write!(f, "Coordinator rejected: {reason:?}"),
            Self::Stage(stage) => {
                write!(f, "Native main stage failed: {:?} ({:?})", stage.stage, stage.failure)
            }
            _ => f.write_str("Production profile transaction rejected"),
        }
    }
}
impl Error for ProfileError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Claim(e) => Some(e),
            Self::Allocation(e) => Some(e),
            Self::Protocol(e) => Some(e),
            Self::Configuration(e) => Some(e),
            Self::Request(e) => Some(e),
            _ => None,
        }
    }
}
impl From<ProtocolError> for ProfileError {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
    }
}
/// Return the original authenticated read guard; error handling cannot imply an ACK or retirement.
pub(in super::super) struct ProfileReplyFailure {
    pub(in super::super) frame: ReceivedLifecycleFrame,
    pub(in super::super) error: Box<dyn Error>,
}
impl fmt::Debug for ProfileReplyFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProfileReplyFailure").finish_non_exhaustive()
    }
}
impl From<ConfigurationError> for ProfileError {
    fn from(e: ConfigurationError) -> Self {
        Self::Configuration(e)
    }
}
impl From<sugarsubstitute_desktop::MainRequestError> for ProfileError {
    fn from(e: sugarsubstitute_desktop::MainRequestError) -> Self {
        Self::Request(e)
    }
}
