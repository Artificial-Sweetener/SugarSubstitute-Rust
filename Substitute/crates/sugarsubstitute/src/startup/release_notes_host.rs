// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route the page's release notes requests and fetch completions through the startup host.
//!
//! The host answers from its release notes page link, starts fetches on its own execution
//! scope while it is open, and asks a running fetch to stop once it closes.
use super::*;

impl<B: StartupBackend> StartupHost<B> {
    /// Answer the page's release notes requests once their fetch ends, and ask a fetch to stop
    /// once the application closes.
    pub(super) fn advance_release_notes(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(jobs) = &mut self.release_notes_jobs else {
            return Ok(());
        };
        if self.closing {
            jobs.cancel();
        }
        let client = self.execution.client()?;
        self.release_notes.advance(&mut jobs.port(client, &self.jobs))
    }

    /// Consume the page's release notes requests and retire its session with its navigation;
    /// returns any other observation unchanged. A closing application starts no fetch.
    pub(super) fn observe_release_notes(
        &mut self,
        observation: DesktopStartupObservation,
    ) -> Result<Option<DesktopStartupObservation>, Box<dyn Error>> {
        match observation {
            DesktopStartupObservation::ReleaseNotesMessage(observed) => {
                let (identity, message, lease, claim) = observed.into_parts();
                let client = self.execution.client();
                match (&mut self.release_notes_jobs, client) {
                    (Some(jobs), Ok(client)) if !self.closing => {
                        let mut fetches = jobs.port(client, &self.jobs);
                        self.release_notes.observe(
                            identity,
                            message,
                            lease,
                            claim,
                            &mut fetches,
                        )?;
                    }
                    _ => {
                        let mut fetches = super::release_notes_fetch::NoFetches;
                        self.release_notes.observe(
                            identity,
                            message,
                            lease,
                            claim,
                            &mut fetches,
                        )?;
                    }
                }
                Ok(None)
            }
            other => {
                if let DesktopStartupObservation::MainRetired(identity)
                | DesktopStartupObservation::MainAbsent(identity)
                | DesktopStartupObservation::MainReloadRequested { identity, .. } = &other
                {
                    self.release_notes.retire(*identity);
                }
                Ok(Some(other))
            }
        }
    }
}
