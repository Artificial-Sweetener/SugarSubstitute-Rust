// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check for updates on the schedule after readiness and retain each helper family through cleanup.
use super::*;
use crate::updates::{
    parent::Parent,
    schedule::{CheckOutcome, CheckSchedule},
};
use sugarsubstitute_contracts::lifecycle::CoordinatorKey;
use sugarsubstitute_supervision::setup_storage::RunningApplication;
impl<C: EntryActivation> EntryRunner<C> {
    /// Every updater job uses this runner's actual executor and globally shared identity sequences.
    /// A check starts when the schedule is due; its settled outcome plans the next one. Closing
    /// stops scheduling, and an already running helper follows the application's close.
    pub(super) fn advance_updates(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<(), Box<dyn Error>> {
        let normal = self.normal_shutdown && self.failure_source().is_none();
        if let Some(updater) = &mut self.updater {
            if self.closing {
                updater
                    .application_closing(self.shutdown, normal)
                    .map_err(|cause| cause as Box<dyn Error>)?;
            }
            updater.advance(batch, &self.jobs).map_err(|cause| cause as Box<dyn Error>)?;
            if let Some(progress) = updater.attempt_progress() {
                self.update_status.observe(progress);
            }
        }
        self.update_status.poll();
        let (revision, status) = self.update_status.snapshot();
        // Offered every turn, so a desktop child that starts later still receives it.
        self.child.publish_update_status(revision, status)?;
        if self.closing
            && let Some(scope) = &self.update_status_scope
        {
            // A record read is advisory; closing never waits for one beyond its cancellation.
            scope.cancel();
        }
        if self.closing || !crate::updates::configured() {
            self.update_wake = None;
            return Ok(());
        }
        let Some((key, application)) =
            self.authority.as_ref().and_then(CoordinatorAuthority::update_source)
        else {
            return Ok(());
        };
        let installation =
            sugarsubstitute_configuration::layout::InstallationLayout::for_image_directory(
                application.directory(),
            )
            .root()
            .to_path_buf();
        let now = self.client()?.now()?;
        let schedule = match &mut self.update_schedule {
            Some(schedule) => schedule,
            None => self.update_schedule.insert(CheckSchedule::new(now)?),
        };
        if let Some((outcome, attempt)) = self.updater.as_mut().and_then(Parent::take_check_outcome)
        {
            self.updater = None;
            tracing::info!(operation = "update_check_finished", outcome = ?attempt,
                "Update check finished");
            schedule.finish(outcome, now)?;
            self.update_status.finish(attempt, crate::updates::status::now_utc());
        }
        if self.updater.is_none() && schedule.begin(now) {
            match self.start_update_check(key, application) {
                Ok(mut updater) => {
                    updater.advance(batch, &self.jobs).map_err(|cause| cause as Box<dyn Error>)?;
                    self.updater = Some(updater);
                    self.update_status.begin();
                }
                Err(cause) => {
                    tracing::warn!(operation="update_helper_admission_failed", cause=%crate::failure_details::Details(cause.as_ref()), "Application is ready; background preparation was not admitted");
                    self.update_schedule
                        .as_mut()
                        .ok_or("Update schedule absent")?
                        .finish(CheckOutcome::Failed, now)?;
                }
            }
        }
        if self.update_status.wants_read() {
            let client = self.client()?.clone();
            if self.update_status_scope.is_none() {
                self.update_status_scope = Some(client.create_scope(
                    ScopeSpec { id: self.scopes.allocate()?, deadline: None },
                    None,
                )?);
            }
            let scope = self.update_status_scope.as_ref().ok_or("Update status scope absent")?;
            self.update_status.submit_read(&client, scope, self.jobs.allocate()?, installation)?;
        }
        self.update_wake = match self.update_schedule.as_ref().and_then(CheckSchedule::due) {
            Some(due) => Some(
                (self.native_now)()
                    .checked_add(due.checked_duration_since(now).unwrap_or_default())
                    .ok_or("Update check wake overflow")?,
            ),
            None => None,
        };
        Ok(())
    }

    /// Admit one helper family in fresh scopes; failure leaves no partial helper behind.
    fn start_update_check(
        &self,
        key: CoordinatorKey,
        application: Arc<RunningApplication>,
    ) -> Result<Parent, Box<dyn Error>> {
        let client = self.client()?.clone();
        let work =
            client.create_scope(ScopeSpec { id: self.scopes.allocate()?, deadline: None }, None)?;
        let cleanup =
            client.create_scope(ScopeSpec { id: self.scopes.allocate()?, deadline: None }, None)?;
        let native = ScopeSpec { id: self.scopes.allocate()?, deadline: None };
        Parent::new(client, work, cleanup, native, key, application)
            .map_err(|cause| cause as Box<dyn Error>)
    }
}
