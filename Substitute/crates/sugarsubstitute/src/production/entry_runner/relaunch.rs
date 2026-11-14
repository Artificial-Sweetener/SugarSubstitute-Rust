// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Start the installation's entry once more after a restart to apply an update, and keep a
//! waiting surface on screen until the new copy shows its first window.
//!
//! The desktop child asks for this only after its close save finished. The runner acts only when
//! that child then completed a normal close and the coordinator released the installation, so
//! the new process starts against a free installation and its own launch picks the prepared
//! version. A beta-eager launch that prepared an update before its window opened closes and
//! relaunches the same way, without asking. A crash, a failure or a close without that intent
//! never relaunches.
//!
//! From the moment the restart is owed, the runner shows the splash's "Updating to version X…"
//! notice. A window that restarts before it was ever shown keeps its own splash until that
//! notice is on screen, and the notice stays until a visible window of the new copy is observed,
//! or the relaunch failed, so the user never looks at an empty screen between the two copies.
use super::*;
use std::os::windows::ffi::OsStrExt;
use sugarsubstitute_contracts::execution::{
    JobClass, JobOutcome, JobSpec, Moment, Operation, OwnedJob,
};
use sugarsubstitute_supervision::{
    WindowsLaunchCommand,
    application_launch::{ApplicationLaunch, CREATION_RESERVATION_BYTES},
};

/// How often the runner looks for the new copy's first window while it waits.
pub(super) const HANDOFF_OBSERVATION: Duration = Duration::from_millis(100);

/// How long starting the stable entry may take when a close ran past its cutoff.
const RELAUNCH_LIMIT: Duration = Duration::from_secs(30);

/// How long the restart notice's frame jobs may run; far beyond any restart.
const NOTICE_JOBS: Duration = Duration::from_secs(120);

/// Progress of the one relaunch.
#[derive(Default)]
pub(super) enum Relaunch {
    /// Not started; it may never start.
    #[default]
    Idle,
    /// The launch job is running; the system time it began, for the new copy's windows.
    Launching(OwnedJob<(), sugarsubstitute_supervision::ClaimError>, u64),
    /// The new copy started at or after this system time; its first window is awaited.
    Launched(u64),
    /// The new copy shows a window, or the launch failed; it is never retried.
    Done,
}

/// The coordinator's "Updating to version X…" notice while it restarts, with its own scope.
pub(super) struct RestartNotice {
    pub(super) notice: crate::waiting_notice::Notice,
    scope: Scope,
    until: Moment,
}

impl RestartNotice {
    /// The native wake at the notice moment while it is still waiting.
    pub(super) fn next_wake(&self) -> Option<Instant> {
        self.notice.next_wake()
    }
}

/// Whether the waiting notice is still owed for `relaunch`: until the new copy's window shows
/// or the launch failed.
pub(super) fn notice_owed(restart_owed: bool, relaunch: &Relaunch) -> bool {
    match relaunch {
        Relaunch::Idle => restart_owed,
        Relaunch::Launching(..) | Relaunch::Launched(_) => true,
        Relaunch::Done => false,
    }
}

/// The installation's stable entry and root for the directory this run's image executes from.
/// An installed run executes from its generation's folder below `releases`; relaunching the
/// stable entry routes to the prepared update, while that folder's own image would start this
/// same generation again.
fn stable_entry(image_directory: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let layout = sugarsubstitute_configuration::layout::InstallationLayout::for_image_directory(
        image_directory,
    );
    (layout.entry(), layout.root().to_path_buf())
}

impl<C: EntryActivation> EntryRunner<C> {
    /// Whether this run's close owes a relaunch into a prepared update: a restart the window
    /// asked for, or one decided before the window opened. However that close ends, normally or
    /// with a failure, the relaunch is owed, so the user is never left without the app: the
    /// stable launcher then starts the prepared version, or the accepted one. The window's ask
    /// is remembered, so the restart stays owed, and its notice on screen, after the closed
    /// window's record is gone and before the relaunch starts.
    pub(super) fn relaunch_wanted(&self) -> bool {
        if self.child.restart_to_apply_requested() {
            self.restart_asked.set(true);
        }
        self.restart_asked.get() || self.launch_update.restart().is_some()
    }

    /// Whether a restart into an update is owed now: decided before the window opened, or asked
    /// for by the window ("Restart to apply") from the moment it starts closing.
    pub(super) fn restart_pending(&self) -> bool {
        (self.launch_update.restart().is_some()
            && !self.authority.as_ref().is_some_and(CoordinatorAuthority::window_opened))
            || self.relaunch_wanted()
    }

    /// Start the owed relaunch now if it has not started, for a close whose cleanup ran past
    /// its cutoff: execution then stops admitting work, and the user still gets the app back.
    /// # Errors
    /// Returns admission and clock failures of the launch job.
    pub(super) fn begin_owed_relaunch(&mut self) -> Result<(), Box<dyn Error>> {
        if matches!(self.relaunch, Relaunch::Idle) && self.relaunch_wanted() {
            let until = self.client()?.now()?.checked_add(RELAUNCH_LIMIT)?;
            self.submit_relaunch(until)?;
        }
        Ok(())
    }

    /// Whether the relaunch job runs now; execution keeps admitting work until it returned.
    pub(super) fn relaunch_running(&self) -> bool {
        matches!(self.relaunch, Relaunch::Launching(..))
    }

    /// Take a started relaunch's result without waiting for the new copy's window, for a close
    /// that only observes execution retirement now; its job never stays unacknowledged.
    pub(super) fn take_relaunch_result(&mut self) {
        if let Relaunch::Launching(job, _) = &mut self.relaunch
            && let Some(completion) = job.try_take_completion()
        {
            if !matches!(completion.outcome, JobOutcome::Success(())) {
                tracing::error!(operation = "restart_to_apply_relaunch_failed", outcome = ?completion.outcome.kind(),
                    "SugarSubstitute could not be started again; the update applies at the next start");
            }
            self.relaunch = Relaunch::Done;
        }
    }

    /// Advance the relaunch; true once nothing more is owed. Call only after the installation
    /// was released. A failed launch is logged and never retried; a started one is awaited until
    /// the new copy shows a window.
    /// # Errors
    /// Returns admission and clock failures of the launch job.
    pub(super) fn advance_relaunch(&mut self, until: Moment) -> Result<bool, Box<dyn Error>> {
        if matches!(self.relaunch, Relaunch::Idle) {
            if !self.relaunch_wanted() {
                return Ok(true);
            }
            self.submit_relaunch(until)?;
        }
        if let Relaunch::Launching(job, since) = &mut self.relaunch {
            let since = *since;
            let Some(completion) = job.try_take_completion() else { return Ok(false) };
            self.relaunch = match completion.outcome {
                JobOutcome::Success(()) => Relaunch::Launched(since),
                JobOutcome::Failure(cause) => {
                    tracing::error!(
                        operation = "restart_to_apply_relaunch_failed",
                        cause = %cause,
                        "SugarSubstitute could not be started again; the update applies at the next start"
                    );
                    Relaunch::Done
                }
                other => {
                    tracing::error!(
                        operation = "restart_to_apply_relaunch_failed",
                        outcome = ?other.kind(),
                        "SugarSubstitute could not be started again; the update applies at the next start"
                    );
                    Relaunch::Done
                }
            };
        }
        if let Relaunch::Launched(since) = self.relaunch {
            match sugarsubstitute_desktop::application_window_shown_since(
                crate::APPLICATION_NAME,
                since,
            ) {
                Ok(false) => return Ok(false),
                Ok(true) => tracing::info!(
                    operation = "restart_to_apply_handoff",
                    "The new copy shows its first window; this copy's notice closes"
                ),
                Err(cause) => tracing::error!(
                    operation = "restart_to_apply_handoff_unobserved",
                    cause = %cause,
                    "The new copy's window could not be looked for; this copy's notice closes"
                ),
            }
            self.relaunch = Relaunch::Done;
        }
        Ok(true)
    }

    /// Start the stable entry once, so the prepared update, or the accepted version, starts.
    /// # Errors
    /// Returns admission and clock failures of the launch job.
    fn submit_relaunch(&mut self, until: Moment) -> Result<(), Box<dyn Error>> {
        let (entry, root) = stable_entry(&self.installation);
        let command = WindowsLaunchCommand::from_utf16(
            &entry.as_os_str().encode_wide().collect::<Vec<_>>(),
            &root.as_os_str().encode_wide().collect::<Vec<_>>(),
            &[],
        )?;
        let launch = ApplicationLaunch::new(command);
        let client = self.client()?;
        let cleanup = self.cleanup.as_ref().ok_or("Production cleanup scope missing")?;
        let since = sugarsubstitute_desktop::system_time_100ns();
        let job = client.submit_blocking(
            cleanup,
            JobSpec {
                id: self.jobs.allocate()?,
                operation: Operation::Infrastructure,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(until),
                reservation_bytes: CREATION_RESERVATION_BYTES,
                replacement: None,
            },
            move |context| {
                let launched = launch.launch(&context);
                if launched.is_err() {
                    launch.retire()?;
                }
                launched
            },
        )?;
        tracing::info!(
            operation = "restart_to_apply_relaunch",
            "Starting SugarSubstitute again so the prepared update applies"
        );
        self.relaunch = Relaunch::Launching(job, since);
        Ok(())
    }

    /// The native wake that looks for the new copy's first window while it is awaited.
    pub(super) fn relaunch_wake(&self) -> Option<Instant> {
        matches!(self.relaunch, Relaunch::Launched(_))
            .then(|| (self.native_now)().checked_add(HANDOFF_OBSERVATION))
            .flatten()
    }

    /// Show the restart notice while a restart is owed and until the new copy's window shows;
    /// drain its jobs once it is no longer owed. True while a notice job still runs.
    /// # Errors
    /// Returns execution admission and clock failures of the notice's scope.
    pub(super) fn advance_restart_notice(&mut self) -> Result<bool, Box<dyn Error>> {
        let owed = notice_owed(self.restart_pending(), &self.relaunch);
        if owed && self.restart_notice.is_none() {
            let client = self.client()?;
            let now = client.now()?;
            let scope = client
                .create_scope(ScopeSpec { id: self.scopes.allocate()?, deadline: None }, None)?;
            self.restart_notice = Some(RestartNotice {
                notice: crate::waiting_notice::Notice::immediate(now),
                scope,
                until: now.checked_add(NOTICE_JOBS)?,
            });
        }
        let version = self
            .launch_update
            .restart()
            .map(str::to_owned)
            .or_else(|| self.update_status.snapshot().1.ready().map(str::to_owned));
        let now = self.client()?.now()?;
        let Some(restart) = &mut self.restart_notice else { return Ok(false) };
        restart.notice.show_update(version.as_deref());
        restart.notice.observe(now, owed);
        if !owed {
            restart.notice.cancel();
        }
        let client = self.client.as_ref().ok_or("Production client missing")?;
        restart.notice.advance(
            &crate::waiting_notice::Resources {
                client,
                scope: &restart.scope,
                jobs: &self.jobs,
                until: restart.until,
            },
            !owed,
        );
        Ok(restart.notice.busy())
    }

    /// Whether a window that restarts may close now: this run's restart notice is on screen to
    /// stay until the new copy shows, or it can never show. A SugarCubes install that runs now
    /// finishes first: the window keeps the screen until then, so the restart never cuts the
    /// install short and its result is reported as it is.
    pub(super) fn restart_covered(&self) -> bool {
        !self.child.sugarcubes_installing()
            && self
                .restart_notice
                .as_ref()
                .is_some_and(|restart| restart.notice.on_screen_or_unavailable())
    }

    /// A turn at once while the window has not yet been told the restart is covered.
    pub(super) fn restart_cover_wake(&self) -> Option<Instant> {
        (self.restart_covered() && !self.update_status.snapshot().1.restart_covered())
            .then(|| (self.native_now)())
    }

    /// The restart notice for the pump while it is shown.
    pub(super) fn restart_notice(&mut self) -> Option<sugarsubstitute_desktop::HeadlessNotice> {
        self.restart_notice.as_mut().and_then(|restart| restart.notice.wanted())
    }

    /// The pump drew the restart notice's window; prepare frames for it.
    pub(super) fn restart_notice_surface(
        &mut self,
        surface: sugarsubstitute_desktop::NoticeSurface,
    ) {
        if let Some(restart) = &mut self.restart_notice {
            restart.notice.surface(surface);
        }
        self.observe_restart_notice();
    }

    /// Advance the restart notice; a failure is logged and never affects the restart. True while
    /// a notice job still runs.
    pub(super) fn observe_restart_notice(&mut self) -> bool {
        match self.advance_restart_notice() {
            Ok(busy) => busy,
            Err(cause) => {
                tracing::warn!(operation = "restart_notice", cause = %crate::failure_details::Details(cause.as_ref()),
                    "The restart notice could not be prepared; the restart continues without it");
                if let Some(restart) = &mut self.restart_notice {
                    restart.notice.abandon();
                }
                self.restart_notice.as_ref().is_some_and(|restart| restart.notice.busy())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Relaunch, notice_owed, stable_entry};
    use std::path::Path;

    /// An installed run relaunches the root's stable entry, which routes to the prepared update,
    /// never its own generation's image.
    #[test]
    fn an_installed_run_relaunches_the_stable_entry_at_the_installation_root() {
        let root = Path::new(r"D:\Apps\SugarSubstitute");
        let (entry, directory) = stable_entry(&root.join("releases").join("0123abcd"));
        assert_eq!(entry, root.join("SugarSubstitute.exe"));
        assert_eq!(directory, root);
    }

    /// The notice is owed from the restart decision through the launch until the new copy's
    /// window shows; a started copy keeps it even though nothing more is owed by this run.
    #[test]
    fn the_restart_notice_stays_until_the_new_copy_shows_a_window() {
        assert!(!notice_owed(false, &Relaunch::Idle));
        assert!(notice_owed(true, &Relaunch::Idle));
        assert!(notice_owed(false, &Relaunch::Launched(7)), "awaiting the new copy's window");
        assert!(!notice_owed(true, &Relaunch::Done), "shown or failed: the notice closes");
    }
}
