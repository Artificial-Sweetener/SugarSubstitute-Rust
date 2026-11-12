// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Finalize trial selection only after the coordinator's actual native family has retired.
use super::*;

impl Ownership {
    /// Original live trial custody needs a successful final publication before release.
    pub(in super::super) fn settled(&self) -> bool {
        !self.pending()
            && ((self.settlement.unissued()
                && self.application().is_none_or(|application| application.trial().is_none()))
                || self.settlement.succeeded())
    }
}

impl CoordinatorAuthority {
    /// Called after checked claims closure and an empty process registry, with launch admission
    /// closed. Uses the original cleanup cutoff and retains original trial custody until observed.
    pub(in super::super) fn settle_installation(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
    ) -> Result<(), AuthorityIssue> {
        if !self.closing
            || !self.closed
            || self.pending()
            || self.registry().is_some_and(|registry| !registry.is_empty())
        {
            return Err(AuthorityIssue::Pending);
        }
        if self.installation.settled() {
            return Ok(());
        }
        if !self.installation.settlement.unissued() {
            return Err(AuthorityIssue::Stopped);
        }
        let until = self.close_until.ok_or(AuthorityIssue::Pending)?;
        let spec = self.spec(
            client,
            scope,
            jobs,
            setup_storage::SELECTION_PUBLICATION_RESERVATION_BYTES,
            until,
        )?;
        let application =
            self.installation.application().cloned().ok_or(AuthorityIssue::Identity)?;
        let verdict = self.installation.verdict();
        self.installation
            .settlement
            .start(client, scope, spec, application, publication::Decision::Finish(verdict))
            .map_err(|cause| {
                self.fail(Failure::InstallationAdmission(cause));
                AuthorityIssue::Adapter
            })
    }
}

impl CoordinatorAuthority {
    /// Windows is ending the session: record it for the verdict and, for a trial not yet
    /// accepted, publish the verdict at once, before the close save. That is no verdict unless
    /// the host's own failure was observed first. Windows may end this process before the
    /// ordinary close settles; the published verdict then stands for the custody witness too.
    pub(in crate::production) fn observe_session_end(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
    ) -> Result<(), AuthorityIssue> {
        self.observe_trial_close(sugarsubstitute_contracts::installation::CloseCause::SessionEnd);
        let installation = &self.installation;
        let Some(application) = installation.application().cloned() else { return Ok(()) };
        if application.trial().is_none()
            || installation.health.succeeded()
            || !installation.session_end.unissued()
        {
            return Ok(());
        }
        let spec = self.spec(
            client,
            scope,
            jobs,
            setup_storage::SELECTION_PUBLICATION_RESERVATION_BYTES,
            crate::job_deadline::OPEN_ENDED,
        )?;
        let decision = publication::Decision::Finish(self.installation.verdict());
        if let Err(cause) =
            self.installation.session_end.start(client, scope, spec, application, decision)
        {
            tracing::warn!(
                operation = "installation_trial_session_end_unsettled",
                cause = %cause,
                "The session-end settlement could not start; the ordinary settlement still runs"
            );
        }
        Ok(())
    }
}
