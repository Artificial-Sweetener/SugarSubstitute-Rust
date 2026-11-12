// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Project the channel's own pipe, write-cutoff and UI challenge errors into closed categories.
use super::{LifecyclePipeError, challenge::ChallengeError, expiry::NativeWriteExpiry};
use std::error::Error;
use sugarsubstitute_desktop::{
    StartupCauseCode as Code, StartupChallengeCause as Challenge, StartupPipeCause as Pipe,
};

/// Name a channel-owned cause by its concrete type; other causes stay with their owners.
pub(in crate::production::child) fn cause_code(cause: &(dyn Error + 'static)) -> Option<Code> {
    if cause.downcast_ref::<NativeWriteExpiry>().is_some() {
        return Some(Code::Pipe(Pipe::DeadlineExpired));
    }
    if let Some(pipe) = cause.downcast_ref::<LifecyclePipeError>() {
        return Some(Code::Pipe(match pipe {
            LifecyclePipeError::Closed => Pipe::Closed,
            LifecyclePipeError::Busy => Pipe::Busy,
            LifecyclePipeError::NotReady => Pipe::NotReady,
            LifecyclePipeError::InvalidOperation => Pipe::InvalidOperation,
            LifecyclePipeError::DeadlineExpired => Pipe::DeadlineExpired,
            LifecyclePipeError::InvalidDeadline => Pipe::InvalidDeadline,
            LifecyclePipeError::InvalidMessage => Pipe::InvalidMessage,
            LifecyclePipeError::AlreadyTransferred => Pipe::AlreadyTransferred,
            // Only helper purpose channels have a direction rule; a desktop lifecycle channel
            // never produces this, and an operation the side may not issue is invalid.
            LifecyclePipeError::Direction => Pipe::InvalidOperation,
        }));
    }
    cause.downcast_ref::<ChallengeError>().map(|challenge| {
        Code::Challenge(match challenge {
            ChallengeError::Registration => Challenge::Registration,
            ChallengeError::Closed => Challenge::Closed,
            ChallengeError::Identity => Challenge::Identity,
            ChallengeError::Busy => Challenge::Busy,
            ChallengeError::Replay => Challenge::Replay,
            ChallengeError::Order => Challenge::Order,
            ChallengeError::Rejected => Challenge::Rejected,
        })
    })
}

#[cfg(test)]
#[path = "diagnostic_tests.rs"]
mod tests;
