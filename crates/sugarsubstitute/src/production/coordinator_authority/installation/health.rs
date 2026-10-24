// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind authenticated readiness to the owned installation publication.
use super::*;

impl CoordinatorAuthority {
    /// Called only after current native main/browser/backend authorization accepted its reveal.
    /// The finite storage deadline begins here, after readiness, and never bounds provisioning.
    pub(crate) fn accept_installation(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
    ) -> Result<(), AuthorityIssue> {
        if !self.serving() {
            return Err(AuthorityIssue::Closed);
        }
        self.installation.observe_readiness();
        if !self.installation.needs_health() {
            self.installation.publish_shell(client, scope, jobs);
            return Ok(());
        }
        let now = client.now().map_err(|cause| {
            self.fail(Failure::Clock(cause));
            AuthorityIssue::Clock
        })?;
        let until = now
            .checked_add(std::time::Duration::from_secs(30))
            .map_err(|_| AuthorityIssue::Clock)?;
        let spec = self.spec(
            client,
            scope,
            jobs,
            setup_storage::SELECTION_PUBLICATION_RESERVATION_BYTES,
            until,
        )?;
        self.installation.accept_ready(client, scope, spec).map_err(|cause| {
            self.fail(Failure::InstallationAdmission(cause));
            AuthorityIssue::Adapter
        })
    }
}
