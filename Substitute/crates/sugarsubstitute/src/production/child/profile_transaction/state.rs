// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Match immutable transaction facts before changing any actual desktop stage.
use super::*;
use sugarsubstitute_supervision::lifecycle_protocol::{
    MAX_FOLDER_FRAGMENT_UNITS, MAX_PROFILE_FOLDER_UNITS, ReplyOutcome,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Phase {
    Requested,
    EnvironmentWaiting,
    Environment,
    ControllerWaiting,
    Browser,
    Authorized,
    Closed,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Pending {
    pub(super) header: WireHeader,
    pub(super) admitted: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Action {
    Advance(usize),
    PrepareMain,
    AuthorizeEnvironment,
    AuthorizeBrowser,
}
pub(super) struct Progress {
    pub(super) identity: MainIdentity,
    pub(super) original: Moment,
    pub(super) phase: Phase,
    pub(super) offset: usize,
    pub(super) total: usize,
}
impl Progress {
    pub(super) fn new(
        identity: MainIdentity,
        original: Moment,
        total: usize,
    ) -> Result<Self, ProfileError> {
        if !(1..=MAX_PROFILE_FOLDER_UNITS).contains(&total) {
            return Err(ProfileError::Folder);
        }
        Ok(Self { identity, original, phase: Phase::Requested, offset: 0, total })
    }
    pub(super) fn next_offset(&self) -> usize {
        self.total.min(self.offset + MAX_FOLDER_FRAGMENT_UNITS)
    }
    /// Validate matching data only. The caller obtains the actual authenticated frame and
    /// consumes its real request ledger before applying this inert transition.
    pub(super) fn reply(
        &self,
        pending: Pending,
        header: WireHeader,
        reply: ReplyOutcome<'_>,
        browser: Option<NonZeroU32>,
    ) -> Result<Action, ProfileError> {
        if self.phase == Phase::Closed {
            return Err(ProfileError::Closed);
        }
        if !pending.admitted || pending.header != header {
            return Err(ProfileError::Identity);
        }
        match (self.phase, reply) {
            (
                Phase::Requested | Phase::Environment,
                ReplyOutcome::FolderChunkAccepted { identity, next_offset_units },
            ) if identity == self.identity && next_offset_units as usize == self.next_offset() => {
                if self.next_offset() < self.total {
                    Ok(Action::Advance(self.next_offset()))
                } else if self.phase == Phase::Requested {
                    Ok(Action::PrepareMain)
                } else {
                    Err(ProfileError::Reply)
                }
            }
            (Phase::Environment, ReplyOutcome::EnvironmentAuthorized(identity))
                if identity == self.identity && self.next_offset() == self.total =>
            {
                Ok(Action::AuthorizeEnvironment)
            }
            (Phase::Browser, ReplyOutcome::BrowserAuthorized { identity, browser_process_id })
                if identity == self.identity && Some(browser_process_id) == browser =>
            {
                Ok(Action::AuthorizeBrowser)
            }
            (_, ReplyOutcome::Failed(code)) => Err(ProfileError::CoordinatorFailed(code)),
            (_, ReplyOutcome::Rejected(code)) => Err(ProfileError::CoordinatorRejected(code)),
            (_, ReplyOutcome::Cancelled) => Err(ProfileError::Cancelled),
            (_, ReplyOutcome::MainStageFailed(failure)) if failure.identity == self.identity => {
                Err(ProfileError::Stage(failure))
            }
            _ => Err(ProfileError::Reply),
        }
    }
    pub(super) fn apply(&mut self, action: Action) {
        match action {
            Action::Advance(offset) => self.offset = offset,
            Action::PrepareMain => self.phase = Phase::EnvironmentWaiting,
            Action::AuthorizeEnvironment => self.phase = Phase::ControllerWaiting,
            Action::AuthorizeBrowser => self.phase = Phase::Authorized,
        }
    }
}
