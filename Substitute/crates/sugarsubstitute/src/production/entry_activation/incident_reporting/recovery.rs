// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep genuine current startup authority separate from historical persisted incident data.
use super::*;
pub(super) struct CurrentReportSource {
    pub(super) run: RunKey,
    pub(super) startup: sugarsubstitute_execution::RegisteredStartupSource,
    pub(super) locale: sugarsubstitute_contracts::configuration::LocaleId,
}
impl IncidentReporting {
    /// Called only from the actual authenticated incoming projection before recovery scan.
    pub(in crate::production::entry_activation) fn configure_recovery_source(
        &mut self,
        run: RunKey,
        startup: sugarsubstitute_execution::RegisteredStartupSource,
        locale: sugarsubstitute_contracts::configuration::LocaleId,
    ) -> Result<()> {
        if let Some(original) = &self.recovery_source {
            if original.run != run
                || !original.startup.same_source(&startup)
                || original.locale != locale
            {
                return Err("Actual current recovered-report startup changed".into());
            }
            return Ok(());
        }
        self.recovery_source = Some(CurrentReportSource { run, startup, locale });
        Ok(())
    }
    /// Recovered subject is data only. Require the retained real current Record/source still matches.
    pub(super) fn recovered_launch(
        &self,
        owner: sugarsubstitute_contracts::lifecycle::CoordinatorKey,
        incident: IncidentKey,
        records: &[Option<RetainedRecord>; 64],
    ) -> Result<(
        sugarsubstitute_execution::RegisteredStartupSource,
        sugarsubstitute_contracts::configuration::LocaleId,
    )> {
        let current = self
            .recovery_source
            .as_ref()
            .ok_or("Genuine current recovered-report source absent")?;
        if current.run.owner != owner
            || incident.subject().coordinator().namespace != owner.namespace
        {
            return Err("Recovered report namespace/current owner changed".into());
        }
        let record = records
            .iter()
            .flatten()
            .find(|held| held.record.run == current.run)
            .ok_or("Current registered recovered-report run absent")?;
        let value = if let Some(start) = &record.record.start {
            start.value()?
        } else {
            match record.record.projected.as_ref().map(|done| &done.outcome) {
                Some(JobOutcome::Success(value)) => value,
                _ => {
                    return Err("Current authenticated recovered-report startup unavailable".into());
                }
            }
        };
        if !value.registered.same_source(&current.startup) || value.locale != current.locale {
            return Err(
                "Actual current registered source differs from recovered report source".into()
            );
        }
        Ok((current.startup.clone(), current.locale))
    }
}
