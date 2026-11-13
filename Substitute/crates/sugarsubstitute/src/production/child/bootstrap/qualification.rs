// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain inert qualification intent and route its genuine registered-fault admission.
use super::*;
impl ChildBootstrap {
    /// Retain bounded inert intent before authentication; it cannot arm a native effect.
    pub(in crate::production::child) fn with_qualification(
        self,
        intent: crate::production::qualification::QualificationIntent,
        run: sugarsubstitute_contracts::lifecycle::RunKey,
    ) -> Self {
        #[cfg(feature = "k1-native-qualification")]
        {
            let mut qualified = self;
            qualified.qualification = super::super::qualification::ChildQualification::new(intent);
            qualified.qualification_run = Some(run);
            qualified
        }
        #[cfg(not(feature = "k1-native-qualification"))]
        {
            let _ = (intent, run);
            self
        }
    }
    /// Advance the original fault mode only after actual capture/parent registration.
    /// # Errors
    /// Retains the original cause through the same Bootstrap capture failure owner.
    #[cfg(feature = "k1-native-qualification")]
    pub(super) fn registered_fault_step(
        &mut self,
    ) -> Result<Option<StartupPreparationStep>, ChildBootstrapIssue> {
        if !self.qualification.fault_requested() {
            return Ok(None);
        }
        if !self.capture.ready() || !self.channel.registered() {
            return Ok(Some(StartupPreparationStep::Waiting));
        }
        let client = self.client.as_ref().ok_or(ChildBootstrapIssue::Identity)?;
        let scope = self.scope.as_ref().ok_or(ChildBootstrapIssue::Identity)?;
        let jobs = self.jobs.as_ref().ok_or(ChildBootstrapIssue::Identity)?;
        let session = self.capture.registered_session().ok_or(ChildBootstrapIssue::Identity)?;
        let run = self.qualification_run.ok_or(ChildBootstrapIssue::Identity)?;
        let until = self.original.ok_or(ChildBootstrapIssue::Identity)?;
        if let Err(cause) =
            self.qualification.advance_fault(client, scope, jobs, session, run, until)
        {
            return Err(self.record(ChildBootstrapIssue::Capture, cause));
        }
        Ok(Some(StartupPreparationStep::Waiting))
    }
}
