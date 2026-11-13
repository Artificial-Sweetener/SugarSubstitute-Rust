// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind authenticated readiness to the owned installation publication.
use super::*;

/// Deadline of a trial's health publication.
///
/// The publication ends on its own storage outcome, an observed failure or cancellation at
/// shutdown; a host that is slow to write after its first window still accepts the trial instead
/// of failing it on elapsed time. Protected storage demands a finite admitted deadline, so the
/// publication carries the open-ended one.
const fn health_publication_deadline() -> Moment {
    crate::job_deadline::OPEN_ENDED
}

impl CoordinatorAuthority {
    /// Called only after current native main/browser/backend authorization accepted its reveal.
    /// The publication begins here, after readiness, and never bounds provisioning. Nothing about
    /// publishing acceptance can close a healthy app: a publication that cannot start or finish
    /// leaves the app open and the trial ends without a verdict.
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
        self.start_health(client, scope, jobs)
    }

    /// Retry a health publication whose admission found execution capacity full, once a
    /// completion batch has released some.
    pub(in super::super) fn retry_health(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
    ) -> Result<(), AuthorityIssue> {
        if self.serving() && self.installation.health_waiting() {
            return self.start_health(client, scope, jobs);
        }
        Ok(())
    }

    fn start_health(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
    ) -> Result<(), AuthorityIssue> {
        let spec = self.spec(
            client,
            scope,
            jobs,
            setup_storage::SELECTION_PUBLICATION_RESERVATION_BYTES,
            health_publication_deadline(),
        )?;
        self.installation.accept_ready(client, scope, spec);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two hours after readiness, far beyond the thirty seconds the publication once had, its
    /// deadline still lies ahead: a slow host's first launch never fails on elapsed time.
    #[test]
    fn trial_health_publication_outlasts_any_elapsed_time_after_readiness() {
        let two_hours_after_readiness = Moment::from_nanos(7_200_000_000_000);
        assert!(health_publication_deadline() > two_hours_after_readiness);
    }
}
