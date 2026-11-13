// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Choose a reauthenticated cached trial or the independent accepted startup path.
use super::*;

impl Launcher {
    /// Every phase borrows the same executor and monotone job sequence; cached work performs no HTTP.
    pub(super) fn advance_cached(&mut self) -> Result<()> {
        let Some(cached) = &mut self.cached else {
            return Ok(());
        };
        let outcome = cached.advance(cached::Resources {
            client: self.client.as_ref().ok_or("Launcher client absent")?,
            work: self.work_scope.as_ref().ok_or("Launcher work scope absent")?,
            cleanup: self.cleanup_scope.as_ref().ok_or("Launcher cleanup scope absent")?,
            jobs: &self.jobs,
            until: self.until.ok_or("Launcher startup cutoff absent")?,
            shutdown: if self.closing { self.cleanup_until } else { None },
            delegate: self.engine.is_none(),
        })?;
        match outcome {
            cached::Outcome::Pending => {}
            cached::Outcome::Unavailable => {
                self.cached = None;
                self.choice = Choice::Accepted;
            }
            cached::Outcome::Candidate(candidate) => {
                self.launch = Some(ApplicationLaunch::trial(Arc::from(candidate)));
                self.cached = None;
                self.choice = Choice::Trial;
            }
            cached::Outcome::Engine(image) => {
                self.delegate = Some(Arc::new(std::sync::Mutex::new(Some(image))));
                self.cached = None;
                self.choice = Choice::Delegate;
            }
        }
        Ok(())
    }

    /// Creation is admitted only after exact selection or trial transfer was observed successful.
    pub(super) fn select_next(&mut self) -> Result<()> {
        let task = match self.choice {
            Choice::Route => Task::Route,
            Choice::Installation => Task::Installation,
            Choice::OpenEngine => Task::OpenEngine,
            Choice::VerifyEngine => Task::VerifyEngine,
            Choice::Delegate => Task::Delegate,
            Choice::Cache => Task::Cache,
            Choice::Accepted => Task::Select,
            Choice::Trial => Task::Trial,
            Choice::Create { optional } => Task::Create { optional },
            Choice::Recover => Task::Recover,
            Choice::Complete => return Ok(()),
        };
        let submitted = match self.choice {
            Choice::Route => {
                let root = self.root.clone();
                let (generation, hops) =
                    self.route.clone().ok_or("Launch engine generation absent")?;
                self.submit(Task::Route, move |context| {
                    setup_storage::route_launch(&root, &generation, hops, &context)
                        .map(Work::Routed)
                        .map_err(io::Error::other)
                })
            }
            Choice::Installation => self.reopen_installation(),
            Choice::OpenEngine => {
                let guard = self.engine.as_ref().ok_or("Receiving engine identity absent")?.clone();
                self.submit(Task::OpenEngine, move |context| {
                    guard.open(&context)?;
                    Ok(Work::EngineOpened)
                })
            }
            Choice::VerifyEngine => {
                let guard = self.engine.as_ref().ok_or("Receiving engine identity absent")?.clone();
                self.submit(Task::VerifyEngine, move |context| {
                    guard.verify(&context)?;
                    Ok(Work::EngineVerified)
                })
            }
            Choice::Delegate => {
                let image = self.delegate.as_ref().ok_or("Newer updater image absent")?.clone();
                let root = self.root.clone();
                self.submit(Task::Delegate, move |context| {
                    let image =
                        image.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
                    let image = image.ok_or_else(|| io::Error::other("Newer updater consumed"))?;
                    let started =
                        sugarsubstitute_supervision::application_launch::start_engine_image(
                            &image.executable(),
                            image.directory(),
                            &root,
                            sugarsubstitute_supervision::application_launch::MAX_ENGINE_HOPS,
                            &context,
                        )
                        .map_err(io::Error::other)?;
                    drop(image);
                    let until = context
                        .now()
                        .checked_add(
                            sugarsubstitute_supervision::application_launch::ENGINE_TAKEOVER,
                        )
                        .map_err(io::Error::other)?;
                    started
                        .await_takeover(&context, until)
                        .map(Work::Delegated)
                        .map_err(io::Error::other)
                })
            }
            Choice::Cache if self.cached.is_none() => {
                let root = self.root.clone();
                self.submit(Task::Cache, move |_| {
                    crate::updates::cached_preparation(root).map(Work::Cache)
                })
            }
            Choice::Accepted => {
                let root = self.root.clone();
                self.submit(Task::Select, move |context| {
                    let reservation = match setup_storage::reserve_installed_launch(&root, &context)
                        .map_err(io::Error::other)?
                    {
                        ReservedLaunch::Application(reserved) => reserved,
                        ReservedLaunch::NeedsInstallation => {
                            return Err(io::Error::other(
                                "Installation has no accepted application; run setup to repair it",
                            ));
                        }
                        ReservedLaunch::ReconcileTrial(_) => {
                            return Err(io::Error::other(
                                "Installation trial requires native process reconciliation",
                            ));
                        }
                    };
                    ApplicationLaunch::selected(reservation.prepare_inheritance(&context)?)
                        .map(Work::Prepared)
                        .map_err(io::Error::other)
                })
            }
            Choice::Trial => {
                let launch = self.launch.as_ref().ok_or("Trial creator absent")?.clone();
                self.submit(Task::Trial, move |context| {
                    crate::authority_clock::authorize(|now| launch.prepare_trial(now, &context))?;
                    Ok(Work::TrialPrepared)
                })
            }
            Choice::Create { optional } => {
                let launch = self.launch.as_ref().ok_or("Application creator absent")?.clone();
                self.submit(Task::Create { optional }, move |context| {
                    launch.launch(&context).map_err(io::Error::other)?;
                    Ok(Work::Launched)
                })
            }
            Choice::Recover => {
                let launch = self.launch.as_ref().ok_or("Recovery creator absent")?.clone();
                self.submit(Task::Recover, move |context| {
                    launch
                        .recover_and_retire(&context)
                        .map(Work::Recovered)
                        .map_err(io::Error::other)
                })
            }
            Choice::Cache | Choice::Complete => Ok(()),
        };
        if let Err(cause) = submitted {
            self.failed(task, cause)?;
            // Optional rejection changed the next action to accepted selection or recovery.
            // Admit that action now: there may be no pending job to produce another wake.
            return self.select_next();
        }
        Ok(())
    }

    /// Optional authority/cache failures never substitute unverified bytes for accepted startup.
    /// A trial may fall back only after recovery proves no native process was created, or that
    /// its child is gone without ever having taken over.
    pub(super) fn failed(&mut self, task: Task, cause: Box<dyn Error>) -> Result<()> {
        if self.closing {
            return Err(cause);
        }
        if matches!(task, Task::Delegate) {
            self.delegated(Takeover::Exited);
            self.update_failure = Some(cause);
            return Ok(());
        }
        if !matches!(
            task,
            Task::Installation | Task::Cache | Task::Trial | Task::Create { optional: true }
        ) {
            return Err(cause);
        }
        tracing::warn!(operation="startup_update_skipped", cause=%crate::failure_details::Details(cause.as_ref()), "Prepared update could not start; recovering creator custody");
        self.update_failure.get_or_insert(cause);
        if matches!(task, Task::Installation | Task::Cache) {
            self.choice = Choice::Accepted;
        } else {
            let now = self.client.as_ref().ok_or("Launcher client absent")?.now()?;
            let until = sugarsubstitute_contracts::lifecycle::ShutdownBudget::new(now)?
                .settle_until
                .min(self.until.ok_or("Launcher cutoff absent")?);
            self.recovery_until = Some(until);
            self.recovery_wake =
                Instant::now().checked_add(until.checked_duration_since(now).unwrap_or_default());
            self.choice = Choice::Recover;
        }
        Ok(())
    }

    /// Actual creation facts survive errors/cancellation; uncertain creation never launches a duplicate.
    pub(super) fn recovered(&mut self, outcome: CreationOutcome) -> Result<()> {
        self.launch = None;
        self.recovery_until = None;
        self.recovery_wake = None;
        match outcome {
            // An updated release that never took over had its trial resolved under this
            // launcher's run, so the accepted release starts now and the update is retried later.
            CreationOutcome::NotCreated | CreationOutcome::Unadopted => {
                self.choice = Choice::Accepted
            }
            CreationOutcome::Created => {
                self.completed = true;
                self.choice = Choice::Complete;
            }
            CreationOutcome::Indeterminate => {
                return Err(self
                    .update_failure
                    .take()
                    .unwrap_or_else(|| "Trial native creation remains uncertain".into()));
            }
        }
        Ok(())
    }
}
