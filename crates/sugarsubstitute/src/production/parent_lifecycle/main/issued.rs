// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify only issued environment/browser privileges against live native profile proofs.
use super::super::{ParentError, ParentLifecycle, ParentResources, Reply, Result};
use sugarsubstitute_application::supervision::MainAuthorizationObservation;
use sugarsubstitute_contracts::execution::Moment;

impl ParentLifecycle {
    /// The transport first checks the exact original native write operation. Ordinary
    /// replies acquire no Main privilege and need no later live-authorization fence.
    pub(in super::super) fn authorization_issued(
        &mut self,
        index: usize,
        resources: &ParentResources<'_, '_>,
    ) -> Result<()> {
        self.apply_issued_authorization(
            index,
            |owner, reply| {
                resources.session.control.validate_current_admission()?;
                let valid = match reply {
                    Reply::Environment(identity) => owner
                        .environment
                        .as_ref()
                        .is_some_and(|proof| proof.validates(resources.profile, identity)),
                    Reply::Browser { identity, pid } => {
                        owner.browser.as_ref().is_some_and(|proof| {
                            proof.validates(resources.profile, identity, pid.get())
                        })
                    }
                    _ => false,
                };
                if valid { Ok(()) } else { Err(ParentError::ForeignPeer.into()) }
            },
            || Ok(resources.execution.client.now()?),
        )
    }

    /// Isolate actual current-admission/profile proof and clock ports. The complete
    /// classification and authoritative Main state transition remain in this owner.
    pub(in super::super) fn apply_issued_authorization(
        &mut self,
        index: usize,
        validate: impl FnOnce(&Self, Reply) -> Result<()>,
        now: impl FnOnce() -> Result<Moment>,
    ) -> Result<()> {
        let record = self.requests.get(index)?;
        let header = record.header;
        if header.run != self.run
            || header.connection != Some(self.connection)
            || header.request.is_none()
        {
            return Err(ParentError::ForeignPeer.into());
        }
        let reply = record.reply.ok_or(ParentError::WrongPhase)?;
        let observation = match reply {
            Reply::Environment(identity) => {
                MainAuthorizationObservation::EnvironmentAuthorizationIssued { identity }
            }
            Reply::Browser { identity, pid } => {
                MainAuthorizationObservation::BrowserAuthorizationIssued {
                    identity,
                    browser_process_id: pid,
                }
            }
            _ => return Ok(()),
        };
        validate(self, reply)?;
        self.main.accept(
            self.request_binding(header)?,
            header.request.ok_or(ParentError::WrongPhase)?,
            observation,
            now()?,
        )?;
        Ok(())
    }
}
