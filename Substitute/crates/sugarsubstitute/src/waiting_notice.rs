// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare a small waiting notice from the splash: the launcher's "Starting…" notice when a
//! launch takes a while, and the coordinator's "Updating to version X…" while it restarts into
//! an update until the next copy shows its first window.
//!
//! [`StartingNotice`] decides from the injected clock when the notice is wanted. Once the
//! desktop pump reports the notice window's surface, one splash frame is prepared for it on
//! the launcher's own executor: the splash without its console and close controls, with the
//! "Starting SugarSubstitute…" line and an idle progress bar. A notice failure is logged and
//! ends the notice; it never affects startup. The launcher runs before any configuration is
//! read, so the notice uses canonical English, as the launcher's startup report does.
use std::{error::Error, time::Instant};
use sugarsubstitute_contracts::{
    execution::{
        ExecutionPort, Generation, JobClass, JobOutcome, JobSpec, Moment, Operation, OwnedJob,
        ReplacementKey, Scope, SurfaceId,
    },
    presentation::{ConsolePresentation, FrameIdentity, FrameShape, PreparedFrame},
    startup::launcher::{LauncherBudget, NoticeChange, NoticePhase, StartingNotice},
};
use sugarsubstitute_desktop::{HeadlessNotice, NoticeSurface};
use sugarsubstitute_execution::ExecutionClient;
use sugarsubstitute_splash::{
    AnimationFrame, Appearance, LOGICAL_SIZE, PreparationError, RenderError,
    SPLASH_CACHE_RESERVATION_BYTES, SplashPreparationResult, SplashRenderRequest, SplashRenderer,
    StartupStatus, Theme, UpdateVersion,
};
use sugarsubstitute_ui::{accent::AccentPalette, progress::ProgressFrame};

/// The launcher's notice copy; see the module documentation.
const LOCALE: &str = "en";

type Initialization = OwnedJob<Result<SplashRenderer, RenderError>, PreparationError>;
type Preparation = OwnedJob<SplashPreparationResult, PreparationError>;

/// What the launcher lends the notice for one turn.
pub(crate) struct Resources<'a> {
    pub client: &'a ExecutionClient,
    pub scope: &'a Scope,
    pub jobs: &'a crate::job_sequence::JobSequence,
    /// The launch's own deadline; notice jobs never outlive it.
    pub until: Moment,
}

pub(crate) struct Notice {
    policy: StartingNotice,
    /// What the notice says: starting, or updating to the version this launch starts.
    status: StartupStatus,
    /// Native wake at the notice moment while it is still waiting.
    wake: Option<Instant>,
    surface: Option<NoticeSurface>,
    initialization: Option<Initialization>,
    preparation: Option<Preparation>,
    /// The frame for the reported surface, until the pump takes it.
    frame: Option<PreparedFrame>,
    /// A frame was already prepared for the reported surface.
    prepared: bool,
    /// The pump took a frame: it shows the window in the same turn.
    presented: bool,
    /// The surface shape and words the running preparation draws.
    preparing_for: Option<(FrameShape, StartupStatus)>,
    failed: bool,
}

impl Notice {
    /// A notice that may appear at the budget's notice moment.
    pub(crate) fn new(budget: LauncherBudget) -> Self {
        Self::waiting(
            StartingNotice::new(budget),
            Instant::now().checked_add(LauncherBudget::NOTICE_DELAY),
        )
    }

    /// A notice that appears as soon as its surface is drawn, from `now` on.
    pub(crate) fn immediate(now: Moment) -> Self {
        Self::waiting(StartingNotice::at(now), Some(Instant::now()))
    }

    fn waiting(policy: StartingNotice, wake: Option<Instant>) -> Self {
        Self {
            policy,
            status: StartupStatus::Starting,
            wake,
            surface: None,
            initialization: None,
            preparation: None,
            frame: None,
            prepared: false,
            presented: false,
            preparing_for: None,
            failed: false,
        }
    }

    /// Say `Updating to version X…` while this launch starts ersion as a trial, or starting
    /// when it starts the accepted version. A change prepares the frame again.
    pub(crate) fn show_update(&mut self, version: Option<&str>) {
        let status = version
            .and_then(UpdateVersion::new)
            .map_or(StartupStatus::Starting, StartupStatus::Updating);
        if status != self.status {
            self.status = status;
            self.prepared = false;
            self.frame = None;
        }
    }

    /// What the notice says now.
    #[cfg(test)]
    pub(crate) const fn status(&self) -> StartupStatus {
        self.status
    }

    /// Observe the clock and whether the launcher still owns startup.
    pub(crate) fn observe(&mut self, now: Moment, owns_startup: bool) {
        if self.policy.observe(now, owns_startup) == Some(NoticeChange::Withdraw) {
            self.frame = None;
        }
        if self.policy.phase() != NoticePhase::Waiting {
            self.wake = None;
        }
    }

    /// Whether the notice is on screen now: the launcher then lets the pump build its loop.
    pub(crate) fn shown(&self) -> bool {
        self.policy.phase() == NoticePhase::Shown && !self.failed
    }

    /// The native wake for the notice moment while it is still waiting.
    pub(crate) const fn next_wake(&self) -> Option<Instant> {
        self.wake
    }

    /// Whether a notice job still runs; the launcher's executor cannot settle before it ends.
    pub(crate) fn busy(&self) -> bool {
        self.initialization.is_some() || self.preparation.is_some()
    }

    /// Stop preparing frames; running jobs observe cancellation and end.
    pub(crate) fn cancel(&self) {
        if let Some(job) = &self.initialization {
            job.cancel();
        }
        if let Some(job) = &self.preparation {
            job.cancel();
        }
    }

    /// The notice for the pump while it is shown; its frame is handed over once.
    pub(crate) fn wanted(&mut self) -> Option<HeadlessNotice> {
        self.shown().then(|| {
            let frame = self.frame.take();
            self.presented |= frame.is_some();
            HeadlessNotice {
                title: crate::APPLICATION_NAME,
                logical_size: [LOGICAL_SIZE; 2],
                frame,
            }
        })
    }

    /// Whether the notice is on the screen, or can never be: the pump took a frame for its
    /// window, or the notice failed.
    pub(crate) fn on_screen_or_unavailable(&self) -> bool {
        (self.presented && self.shown()) || self.failed
    }

    /// End the notice after its owner failed to advance it; running jobs observe cancellation.
    pub(crate) fn abandon(&mut self) {
        self.failed = true;
        self.frame = None;
        self.cancel();
    }

    /// Prepare frames for the window the pump created.
    pub(crate) fn surface(&mut self, surface: NoticeSurface) {
        self.surface = Some(surface);
        self.prepared = false;
        self.frame = None;
    }

    /// Consume finished notice jobs and, while shown, admit the next one. Closing only drains.
    pub(crate) fn advance(&mut self, resources: &Resources<'_>, closing: bool) {
        if let Err(cause) = self.step(resources, closing) {
            tracing::warn!(operation = "launcher_notice", cause = %crate::failure_details::Details(cause.as_ref()),
                "The starting notice could not be prepared; startup continues without it");
            self.abandon();
        }
    }

    fn step(&mut self, resources: &Resources<'_>, closing: bool) -> Result<(), Box<dyn Error>> {
        let mut renderer = None;
        if let Some(job) = &mut self.initialization
            && let Some(completion) = job.try_take_completion()
        {
            self.initialization = None;
            renderer = Some(outcome(completion.outcome)??);
        }
        if let Some(job) = &mut self.preparation
            && let Some(completion) = job.try_take_completion()
        {
            self.preparation = None;
            let result = outcome(completion.outcome)?;
            renderer = Some(result.renderer);
            let frame = result.frame?;
            // A frame drawn for a surface or words that changed meanwhile is never shown; the
            // current ones are prepared next.
            let current = self.surface.map(|surface| (surface.shape, self.status));
            if self.preparing_for.take() == current {
                self.frame = Some(frame);
                self.prepared = true;
            }
        }
        let Some(surface) = self.surface else { return Ok(()) };
        if closing || self.failed || !self.shown() || self.prepared || self.busy() {
            return Ok(());
        }
        let renderer = match renderer {
            Some(renderer) => renderer,
            None => {
                self.initialization = Some(resources.client.submit_cpu(
                    resources.scope,
                    spec(resources, SPLASH_CACHE_RESERVATION_BYTES)?,
                    |context| {
                        SplashRenderer::initialize(LOCALE.to_owned(), context.memory().clone())
                    },
                )?);
                return Ok(());
            }
        };
        let request = request(surface, self.status)?;
        let bytes = request.reservation_bytes();
        self.preparing_for = Some((surface.shape, self.status));
        self.preparation = Some(resources.client.submit_cpu(
            resources.scope,
            spec(resources, bytes)?,
            move |context| renderer.prepare(request, context.memory().clone()),
        )?);
        Ok(())
    }
}

/// The splash, as the launcher's notice, for `surface`.
fn request(
    surface: NoticeSurface,
    status: StartupStatus,
) -> Result<SplashRenderRequest, Box<dyn Error>> {
    Ok(SplashRenderRequest {
        shape: surface.shape,
        identity: FrameIdentity {
            surface: SurfaceId::new(1)?,
            state_generation: Generation::new(1)?,
            sequence: Generation::new(1)?,
        },
        appearance: Appearance {
            theme: if surface.dark { Theme::Dark } else { Theme::Light },
            accent: AccentPalette::resolve(surface.accent),
            status,
            operation: Default::default(),
            expanded: false,
            reveal: 0.0,
            hovered: None,
            pressed: None,
            controls: false,
        },
        character: AnimationFrame::bootstrap(),
        feedback: ProgressFrame { completed: 0.0, activity: None },
        bootstrap: false,
        console: None,
        console_presentation: ConsolePresentation::Native,
    })
}

/// Interactive frame work names its surface; each job is a newer revision of the notice.
fn spec(resources: &Resources<'_>, reservation_bytes: u64) -> Result<JobSpec, Box<dyn Error>> {
    let id = resources.jobs.allocate()?;
    Ok(JobSpec {
        id,
        operation: Operation::FramePrepare,
        class: JobClass::CpuInteractive,
        deadline: Some(resources.until),
        reservation_bytes,
        replacement: Some(ReplacementKey {
            surface: SurfaceId::new(1)?,
            revision: Generation::new(id.get())?,
        }),
    })
}

/// A notice job's typed result, or why it ended without one.
fn outcome<T>(outcome: JobOutcome<T, PreparationError>) -> Result<T, Box<dyn Error>> {
    match outcome {
        JobOutcome::Success(value) => Ok(value),
        JobOutcome::Failure(cause) => Err(Box::new(cause)),
        other => Err(format!("Starting notice preparation ended with {:?}", other.kind()).into()),
    }
}

#[cfg(test)]
#[path = "waiting_notice_tests.rs"]
mod tests;
