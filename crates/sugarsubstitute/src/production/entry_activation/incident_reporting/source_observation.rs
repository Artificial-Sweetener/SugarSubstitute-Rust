// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Separate repository-view admission from observed same-session native authentication.
use super::*;
impl IncidentReporting {
    /// Observe data from the retained source without granting routing or native privileges.
    /// The parent/channel still owns the verified peer and every original failure cause.
    /// Pending authentication cannot renew any original launch, action or cleanup cutoff.
    pub(super) fn observe_source_facts(&mut self, facts: ReporterFacts) -> Result<()> {
        self.relay_needed |= facts.changed;
        if facts.failed {
            self.process = facts.process;
            return Err("Actual Reporter source failed with original cause retained".into());
        }
        let key = self.key.ok_or("Actual Reporter session missing")?;
        if let Some((observed_key, observed_process)) = self.authenticated_peer_observed {
            if key != observed_key || facts.process != Some(observed_process) {
                return Err("Actual authenticated Reporter session or process changed".into());
            }
            if !facts.authenticated {
                return Err("Actual Reporter lost its observed authenticated peer".into());
            }
        } else if facts.authenticated {
            let process = facts.process.ok_or("Actual authenticated Reporter process missing")?;
            if key.role() != DiagnosticRole::Reporter || process.owner != key.owner() {
                return Err("Actual authenticated Reporter owner or role changed".into());
            }
            self.authenticated_peer_observed = Some((key, process));
        }
        self.process = facts.process;
        Ok(())
    }
}
