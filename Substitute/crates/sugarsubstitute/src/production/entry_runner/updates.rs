// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check for updates on the schedule after readiness, and at once when the window asks, and retain
//! each helper family through cleanup.
use super::*;
use crate::updates::{
    parent::Parent,
    schedule::{CheckOutcome, CheckRequest, CheckSchedule},
};
use sugarsubstitute_contracts::{lifecycle::CoordinatorKey, update::status::LaunchVersion};
use sugarsubstitute_supervision::setup_storage::RunningApplication;
impl<C: EntryActivation> EntryRunner<C> {
    /// Every updater job uses this runner's actual executor and globally shared identity sequences.
    /// A check starts when the schedule is due; its settled outcome plans the next one. Closing
    /// stops scheduling, and an already running helper follows the application's close. A check
    /// the window asks for runs the same check now, or joins the one running.
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
        self.update_status.observe_sugarcubes(self.child.sugarcubes_update());
        self.observe_window();
        let check_requested = self.child.take_update_check_request();
        let before_window = self.launch_update.checks_before_window();
        let source =
            self.authority.as_ref().and_then(|authority| authority.update_source(before_window));
        self.update_status
            .observe_checks(!self.closing && crate::updates::configured() && source.is_some());
        self.publish_update_status()?;
        if self.closing
            && let Some(scope) = &self.update_status_scope
        {
            // A record read is advisory; closing never waits for one beyond its cancellation.
            scope.cancel();
        }
        // Every run shows its channel and record, including one that cannot check.
        self.submit_record_read()?;
        let Some((key, application)) =
            source.filter(|_| !self.closing && crate::updates::configured())
        else {
            if check_requested {
                tracing::info!(
                    operation = "update_check_request_declined",
                    "The window asked to check for updates while this launch cannot check"
                );
            }
            if self.closing || !crate::updates::configured() {
                self.update_wake = None;
            }
            return Ok(());
        };
        let now = self.client()?.now()?;
        let schedule = match &mut self.update_schedule {
            Some(schedule) => schedule,
            None => self
                .update_schedule
                .insert(CheckSchedule::new(now, self.launch_update.first_check())?),
        };
        let mut restarting = false;
        if let Some((outcome, attempt)) = self.updater.as_mut().and_then(Parent::take_check_outcome)
        {
            self.updater = None;
            tracing::info!(operation = "update_check_finished", outcome = ?attempt,
                "Update check finished");
            schedule.finish(outcome, now)?;
            self.launch_update.finish(&attempt);
            self.update_status.finish(attempt, crate::updates::status::now_utc());
            restarting = self.launch_update.restart().is_some();
        }
        if check_requested {
            let request = schedule.request(now);
            tracing::info!(
                operation = "update_check_requested",
                joined = request == CheckRequest::Joined,
                "The window asked to check for updates now"
            );
        }
        if !restarting && self.updater.is_none() && schedule.begin(now) {
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
                    self.update_status.not_started(crate::updates::status::now_utc());
                }
            }
        }
        if restarting {
            tracing::info!(
                operation = "update_restart_before_window",
                "An update was prepared before the window opened; restarting into it"
            );
            self.announce_restart()?;
        }
        // A finished check may have prepared an update.
        self.submit_record_read()?;
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

    /// Adopt an opened window. A restart decided before it is withdrawn and the status says so
    /// again, because the app never closes a window the user has seen; the update starts on the
    /// next launch.
    pub(super) fn observe_window(&mut self) {
        if self.authority.as_ref().is_some_and(CoordinatorAuthority::window_opened)
            && self.launch_update.window_opened()
        {
            self.update_status.withdraw_restart();
            tracing::info!(
                operation = "update_restart_withdrawn",
                "The window opened before the restart; the update starts on the next launch"
            );
        }
    }

    /// Tell the window this launch restarts into its prepared update. A window that was never
    /// revealed shows which version it updates to and closes itself to restart.
    pub(super) fn announce_restart(&mut self) -> Result<(), Box<dyn Error>> {
        self.publish_update_status()
    }

    /// Whether this run owes its own restart close now: a restart is decided, the window has not
    /// opened, and no desktop child is live. A live desktop child is never closed from here: one
    /// never revealed closes itself to restart, and one revealed first withdraws the restart.
    pub(super) fn restart_owed(&self) -> bool {
        self.launch_update.restart().is_some()
            && !self.authority.as_ref().is_some_and(CoordinatorAuthority::window_opened)
            && !self.child.has_desktop()
    }

    /// Read this installation's update record and channel when they may have changed and no read
    /// is outstanding. A closing run reads nothing more.
    fn submit_record_read(&mut self) -> Result<(), Box<dyn Error>> {
        if self.closing || !self.update_status.wants_read() {
            return Ok(());
        }
        let client = self.client()?.clone();
        if self.update_status_scope.is_none() {
            self.update_status_scope =
                Some(client.create_scope(
                    ScopeSpec { id: self.scopes.allocate()?, deadline: None },
                    None,
                )?);
        }
        let scope = self.update_status_scope.as_ref().ok_or("Update status scope absent")?;
        let id = self.jobs.allocate()?;
        self.update_status.submit_read(&client, scope, id, record_root(&self.installation))
    }

    /// Offer the coordinator's update status, with what this launch does about its version.
    /// Offered every turn, so a desktop child that starts later still receives it.
    pub(super) fn publish_update_status(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(launch) = self.launch_version() {
            self.update_status.observe_launch(launch);
        }
        let (revision, status) = self.update_status.snapshot();
        self.child.publish_update_status(revision, status)
    }

    /// What this launch did about the version it runs: restarting into its prepared update,
    /// or, when it runs a trial that replaces an accepted release, applying it until the
    /// trial's acceptance is published, then updated to it. An accepted launch, a fresh
    /// installation's first release, and a run without installation ownership start no update.
    pub(super) fn launch_version(&self) -> Option<LaunchVersion> {
        if let Some(version) = self.launch_update.restart() {
            return Some(LaunchVersion::Restarting(version.to_owned()));
        }
        let authority = self.authority.as_ref()?;
        let application = authority.running_installation()?;
        let trial = application.trial()?;
        // Only an accepted trial is an update source; an unaccepted one is still applying.
        trial_launch(
            trial.replaces_release(),
            authority.update_source(false).is_some(),
            application.generation().release.version(),
        )
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
        // Scope identities advance in creation order: the attempt channel opens before the
        // helper's native process scope.
        let channel = ScopeSpec { id: self.scopes.allocate()?, deadline: None };
        let native = ScopeSpec { id: self.scopes.allocate()?, deadline: None };
        Parent::new(client, work, cleanup, native, channel, key, application)
            .map_err(|cause| cause as Box<dyn Error>)
    }
}

/// The installation root whose selection and update trust hold the update record, for the
/// directory this run's image executes from: an installed run executes from its generation's
/// folder below `releases`, which holds no record of its own.
fn record_root(image_directory: &std::path::Path) -> std::path::PathBuf {
    sugarsubstitute_configuration::layout::InstallationLayout::for_image_directory(image_directory)
        .root()
        .to_path_buf()
}

/// A trial's first start of `version`: an update when it replaces an accepted release, applying
/// until `confirmed`; a fresh installation's first release is no update.
fn trial_launch(replaces_release: bool, confirmed: bool, version: &str) -> Option<LaunchVersion> {
    if !replaces_release {
        return None;
    }
    let version = version.to_owned();
    Some(if confirmed { LaunchVersion::Updated(version) } else { LaunchVersion::Applying(version) })
}

#[cfg(test)]
mod tests {
    use super::super::tests::{NoChild, freeze_time, unopened_paths};
    use super::*;

    /// A fresh installation's first release starts without an update notice, before and after
    /// its trial is accepted; a trial that replaces an accepted release applies, then updates.
    #[test]
    fn only_a_trial_that_replaces_an_accepted_release_is_an_update() {
        assert_eq!(trial_launch(false, false, "0.1.0"), None);
        assert_eq!(trial_launch(false, true, "0.1.0"), None);
        assert_eq!(
            trial_launch(true, false, "0.2.0"),
            Some(LaunchVersion::Applying("0.2.0".into()))
        );
        assert_eq!(trial_launch(true, true, "0.2.0"), Some(LaunchVersion::Updated("0.2.0".into())));
    }

    /// An installed run reads its update record and channel from the installation root, not
    /// from the generation folder its image runs from.
    #[test]
    fn the_update_record_is_read_from_the_installation_root() {
        let root = std::path::Path::new(r"D:\Apps\SugarSubstitute");
        assert_eq!(record_root(&root.join("releases").join("0123abcd")), root);
        assert_eq!(record_root(root), root, "a source-tree run keeps its own folder");
    }

    /// A run that cannot check, here one without installation authority, still reads its
    /// update record and channel, so the window shows them.
    #[test]
    fn a_run_that_cannot_check_still_reads_its_record_and_channel() -> Result<(), Box<dyn Error>> {
        let path = std::env::temp_dir().join("unopened-update-record");
        let command = ProductionCommand {
            launch_selection: None,
            installation: path.clone(),
            diagnostics: Default::default(),
            options: unopened_paths()?.into(),
        };
        let mut runner = EntryRunner::new(
            command,
            path,
            Box::new(NoChild(None, false)),
            crate::diagnostics::DiagnosticHealth::fixture(),
        );
        freeze_time(&mut runner);
        runner.initialize_execution(Arc::new(crate::completion_wake::Wake::default()))?;
        assert!(runner.update_status.wants_read());
        let batch = runner.client()?.drain_completions(NonZeroUsize::MIN)?;
        runner.advance_updates(&batch)?;
        assert!(runner.authority.is_none(), "this run cannot check");
        assert!(!runner.update_status.wants_read(), "the record read was submitted anyway");
        Ok(())
    }
}
