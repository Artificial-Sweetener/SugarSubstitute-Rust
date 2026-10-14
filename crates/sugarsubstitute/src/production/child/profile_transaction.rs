// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Gate actual desktop construction on the authenticated coordinator's profile replies.
use super::channel::ChildChannel;
use std::num::NonZeroU32;
use sugarsubstitute_contracts::{
    execution::{JobContext, Moment},
    startup::MainIdentity,
};
use sugarsubstitute_desktop::{DesktopStartupCommand, MainEnvironmentFacts, MainWindowRequest};
use sugarsubstitute_execution::windows_lifecycle_pipe::ReceivedLifecycleFrame;
use sugarsubstitute_supervision::lifecycle_protocol::{FrameCodec, WireHeader};
#[path = "profile_transaction/authorization.rs"]
mod authorization;
#[path = "profile_transaction/errors.rs"]
mod errors;
#[path = "profile_transaction/folder.rs"]
mod folder;
#[path = "profile_transaction/observations.rs"]
mod observations;
#[path = "profile_transaction/requests.rs"]
mod requests;
#[path = "profile_transaction/state.rs"]
mod state;
pub(super) use errors::{ProfileError, ProfileReplyFailure};
use folder::RequestedFolder;
use state::{Pending, Phase, Progress};

/// Own one main attempt's bounded folder transaction and original allocation/deadline custody.
/// Construction runs in the already admitted finite Main-request worker; activity and desktop
/// dispatch remain nonblocking on the existing native event-loop thread.
pub(super) struct ProfileTransaction {
    request: Option<Box<MainWindowRequest>>,
    requested: RequestedFolder,
    environment: Option<MainEnvironmentFacts>,
    browser: Option<NonZeroU32>,
    progress: Progress,
    pending: Option<Pending>,
    context: JobContext,
}
impl ProfileTransaction {
    /// Claim before copying the actual request path in its original finite construction job.
    /// No native/FS work or desktop command occurs here. The original cutoff is never extended.
    /// # Errors
    /// Preserves claim/allocation causes; rejects cancelled, expired or unbounded construction.
    pub(super) fn new(
        request: Box<MainWindowRequest>,
        original: Moment,
        context: &JobContext,
    ) -> Result<Self, ProfileError> {
        check(context, original)?;
        let requested = RequestedFolder::new(&request)?;
        check(context, original)?;
        let progress = Progress::new(request.identity(), original, requested.units.len())?;
        Ok(Self {
            request: Some(request),
            requested,
            environment: None,
            browser: None,
            progress,
            pending: None,
            context: context.clone(),
        })
    }
    /// Return the immutable attempt for routing genuine desktop observations.
    pub(super) const fn identity(&self) -> MainIdentity {
        self.progress.identity
    }
    /// Return the unchanged cutoff for the composing startup/session owner.
    pub(super) const fn original_deadline(&self) -> Moment {
        self.progress.original
    }
    /// Report whether the entire actual profile/browser authorization has completed.
    /// This establishes no frontend/backend readiness or native retirement.
    pub(super) fn authorized(&self) -> bool {
        self.progress.phase == Phase::Authorized
    }
    /// Report whether one actually admitted profile request is awaiting its original reply.
    pub(super) fn awaiting_reply(&self) -> bool {
        self.progress.phase != Phase::Closed && self.pending.is_some_and(|pending| pending.admitted)
    }
    /// Match an admitted original reply header for the composing channel's borrowed routing.
    /// This copied matching data grants no authority and consumes no frame or request slot.
    pub(super) fn expects_reply(&self, header: WireHeader) -> bool {
        self.awaiting_reply() && self.pending.is_some_and(|pending| pending.header == header)
    }
    /// Fence future authorization immediately; retain existing native observation/source custody.
    /// Native desktop and channel retirement are independent checked owners.
    pub(super) fn close(&mut self) {
        self.progress.phase = Phase::Closed;
    }
    fn activity_required(&self) -> Result<bool, ProfileError> {
        if matches!(self.progress.phase, Phase::Authorized | Phase::Closed) {
            return Ok(false);
        }
        self.check()?;
        Ok(matches!(self.progress.phase, Phase::Requested | Phase::Environment | Phase::Browser))
    }
    fn check(&self) -> Result<(), ProfileError> {
        if self.progress.phase == Phase::Closed {
            return Err(ProfileError::Closed);
        }
        check(&self.context, self.progress.original)
    }
}
fn check(context: &JobContext, original: Moment) -> Result<(), ProfileError> {
    if context.cancellation_requested() {
        return Err(ProfileError::Cancelled);
    }
    let now = context.now();
    if context.deadline().is_none_or(|deadline| deadline > original)
        || now >= original
        || context.deadline().is_some_and(|deadline| now >= deadline)
    {
        return Err(ProfileError::Deadline);
    }
    Ok(())
}
#[cfg(test)]
#[path = "profile_transaction/tests.rs"]
mod tests;
