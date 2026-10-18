// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fence a covered Desktop's first presentation behind the Root's actual native hide.
use super::{ParentError, ParentLifecycle, Reply, Result};
use sugarsubstitute_contracts::startup::MainIdentity;

/// Retain the original request slot; it grants no backend or Main-stage authority.
#[derive(Default)]
pub(super) struct SplashHandoff {
    pending: Option<(usize, MainIdentity)>,
    released: bool,
}
impl ParentLifecycle {
    /// Called only after the existing authenticated request admission and cutoff checks.
    pub(super) fn prepare_splash(&mut self, index: usize, identity: MainIdentity) -> Result<()> {
        if self.closed
            || self.cleanup.is_some()
            || self.splash.pending.is_some()
            || self.splash.released
            || identity.run != self.run
            || identity.launch != self.launch
            || self.acquisition.is_some()
            || self.main.snapshot().identity.is_some()
        {
            return Err(ParentError::WrongPhase.into());
        }
        self.requests.get_mut(index)?.identity = Some(identity);
        self.splash.pending = Some((index, identity));
        Ok(())
    }
    /// Borrow the exact covered frame claim while its original request remains pending.
    pub(crate) fn entry_splash_handoff(&self) -> Option<MainIdentity> {
        if self.closed || self.cleanup.is_some() || self.splash.released {
            return None;
        }
        let (index, identity) = self.splash.pending?;
        let request = self.requests.get(index).ok()?;
        (request.identity == Some(identity) && request.reply.is_none()).then_some(identity)
    }
    /// Called on the native composition thread only after the Root surface is actually hidden.
    /// The pending channel request supplies authenticated run/connection/request correlation.
    /// No presentation flag or copied identity authorizes backend or profile work.
    pub(crate) fn complete_entry_splash_handoff(&mut self, identity: MainIdentity) -> Result<()> {
        if self.entry_splash_handoff() != Some(identity) {
            return Err(ParentError::WrongPhase.into());
        }
        let (index, _) = self.splash.pending.ok_or(ParentError::WrongPhase)?;
        self.requests.get_mut(index)?.reply = Some(Reply::Acknowledged);
        self.splash.released = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::production::parent_lifecycle::tests::identity;
    use sugarsubstitute_contracts::configuration::ProfileId;
    use sugarsubstitute_contracts::lifecycle::{ConnectionId, RequestId};
    use sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeDeadline;
    use sugarsubstitute_supervision::lifecycle_protocol::WireHeader;

    type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

    fn header(request: u64) -> Result<WireHeader> {
        Ok(WireHeader {
            run: identity()?.run,
            connection: Some(ConnectionId::new(1)?),
            request: Some(RequestId::new(request)?),
            deadline_qpc: 100,
        })
    }

    fn actor() -> Result<ParentLifecycle> {
        let identity = identity()?;
        let storage = crate::production::coordinator_backend::tests::storage_tests::claim(
            ParentLifecycle::storage_bytes(),
        )
        .map_err(|cause| cause as Box<dyn std::error::Error>)?;
        Ok(ParentLifecycle::new(
            identity.run,
            identity.launch,
            ProfileId::new("Splash_1".into())?,
            ConnectionId::new(1)?,
            LifecyclePipeDeadline::new(100, 1_000_000_000)?,
            &mut Some(crate::production::parent_lifecycle::causes::BackendCauses::new(storage)?),
        )?)
    }

    #[test]
    fn covered_frame_retains_exact_pending_request_until_actual_root_hide_callback() -> Result<()> {
        let mut parent = actor()?;
        let identity = identity()?;
        assert_eq!(parent.entry_splash_handoff(), None);
        assert!(parent.complete_entry_splash_handoff(identity).is_err());
        let (index, _) = parent.requests.admit(header(2)?, b"covered frame")?;
        parent
            .prepare_splash(index, identity)
            .map_err(|cause| cause as Box<dyn std::error::Error>)?;
        assert_eq!(parent.entry_splash_handoff(), Some(identity));
        assert!(parent.requests.get(index)?.reply.is_none());
        let changed = MainIdentity {
            navigation: sugarsubstitute_contracts::execution::Generation::new(2)?,
            ..identity
        };
        assert!(parent.complete_entry_splash_handoff(changed).is_err());
        assert!(parent.requests.get(index)?.reply.is_none());
        parent
            .complete_entry_splash_handoff(identity)
            .map_err(|cause| cause as Box<dyn std::error::Error>)?;
        assert_eq!(parent.entry_splash_handoff(), None);
        assert!(matches!(parent.requests.get(index)?.reply, Some(Reply::Acknowledged)));
        assert_eq!(parent.main.snapshot().identity, None);
        assert!(parent.lease.is_none());
        assert!(parent.complete_entry_splash_handoff(identity).is_err());
        Ok(())
    }

    #[test]
    fn retiring_or_duplicate_covered_splash_never_authorizes_presentation() -> Result<()> {
        let identity = identity()?;
        let mut parent = actor()?;
        let (index, _) = parent.requests.admit(header(2)?, b"covered frame")?;
        let mut foreign = identity;
        foreign.launch = sugarsubstitute_contracts::startup::LaunchId::new(2)?;
        assert!(parent.prepare_splash(index, foreign).is_err());
        assert_eq!(parent.entry_splash_handoff(), None);
        parent
            .prepare_splash(index, identity)
            .map_err(|cause| cause as Box<dyn std::error::Error>)?;
        assert!(parent.prepare_splash(index, identity).is_err());
        parent.closed = true;
        assert_eq!(parent.entry_splash_handoff(), None);
        assert!(parent.complete_entry_splash_handoff(identity).is_err());
        assert!(parent.requests.get(index)?.reply.is_none());
        Ok(())
    }
}
