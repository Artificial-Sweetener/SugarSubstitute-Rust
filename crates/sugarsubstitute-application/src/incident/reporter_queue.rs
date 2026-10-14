// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve ready summaries in their original incident slots until genuine Reporter retirement.
use super::*;
use sugarsubstitute_contracts::incident::*;

impl IncidentCoordinator {
    /// Issue one automatic session without a second queue or additional retained summary.
    /// An admitted unretired session and an unforwarded original restart both fence issuance.
    pub(super) fn append_ready_reporter(&mut self, effects: &mut IncidentEffects) {
        if self.reporter_launch_blocked() {
            return;
        }
        let Some(index) = self.records.iter().position(|record| {
            record.as_ref().is_some_and(|record| {
                record.summary.is_some()
                    && record.binding.is_none()
                    && !record.automatic_report_attempted
                    && record.acknowledgement.is_none()
                    && record.pending.is_none()
                    && record.current.is_none()
            })
        }) else {
            return;
        };
        if effects.len() == MAX_INCIDENT_EFFECTS {
            self.retain_report_refusal(index, IncidentDecisionError::Saturated);
            return;
        }
        let session = match self.session() {
            Ok(session) => session,
            Err(cause) => {
                self.retain_report_refusal(index, cause);
                return;
            }
        };
        let Some(record) = self.records[index].as_ref() else {
            return;
        };
        let Some(summary) = record.summary.as_ref() else {
            return;
        };
        let binding = ReportBinding::new(record.seed.key(), summary.revision(), session);
        if let Err(cause) =
            effects.append(IncidentEffect::LaunchReporter { binding, summary: summary.clone() })
        {
            self.retain_report_refusal(index, cause);
            return;
        }
        let Some(record) = self.records[index].as_mut() else {
            return;
        };
        record.binding = Some(binding);
        record.automatic_report_attempted = true;
        record.reporter_retired = false;
        record.phase = IncidentServicePhase::Reporting;
        self.next_session = session.get();
    }
    fn retain_report_refusal(&mut self, index: usize, cause: IncidentDecisionError) {
        if let Some(record) = self.records[index].as_mut() {
            record.report_admission_failure.get_or_insert(cause);
        }
    }
    /// Preserve current publication admission on session exhaustion when it could launch immediately.
    pub(super) fn preflight_automatic_reporter(&self) -> Result<(), IncidentDecisionError> {
        if !self.reporter_launch_blocked() {
            self.session()?;
        }
        Ok(())
    }
    fn reporter_launch_blocked(&self) -> bool {
        self.records.iter().flatten().any(|record| {
            !record.reporter_retired
                || record.binding.is_some()
                    && (record.pending.is_some()
                        || record.current.is_some()
                        || record.queued_failure.is_some()
                        || record.refresh_pending)
                || record.restart_requested && !record.restart_forwarded
        })
    }
}
