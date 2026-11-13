// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check that channel-owned errors keep their concrete category through the relay projection.
use super::*;

#[test]
fn every_pipe_and_challenge_error_names_its_own_category() {
    for (error, expected) in [
        (LifecyclePipeError::Closed, Pipe::Closed),
        (LifecyclePipeError::Busy, Pipe::Busy),
        (LifecyclePipeError::NotReady, Pipe::NotReady),
        (LifecyclePipeError::InvalidOperation, Pipe::InvalidOperation),
        (LifecyclePipeError::DeadlineExpired, Pipe::DeadlineExpired),
        (LifecyclePipeError::InvalidDeadline, Pipe::InvalidDeadline),
        (LifecyclePipeError::InvalidMessage, Pipe::InvalidMessage),
        (LifecyclePipeError::AlreadyTransferred, Pipe::AlreadyTransferred),
    ] {
        assert_eq!(cause_code(&error), Some(Code::Pipe(expected)));
    }
    for (error, expected) in [
        (ChallengeError::Registration, Challenge::Registration),
        (ChallengeError::Closed, Challenge::Closed),
        (ChallengeError::Identity, Challenge::Identity),
        (ChallengeError::Busy, Challenge::Busy),
        (ChallengeError::Replay, Challenge::Replay),
        (ChallengeError::Order, Challenge::Order),
        (ChallengeError::Rejected, Challenge::Rejected),
    ] {
        assert_eq!(cause_code(&error), Some(Code::Challenge(expected)));
    }
    assert_eq!(cause_code(&std::io::Error::other("foreign")), None);
}
