// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare the launcher's "Starting…" notice from the splash when a launch takes a while.
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
    presentation::{ConsolePresentation, FrameIdentity, PreparedFrame},
    startup::launcher::{LauncherBudget, NoticeChange, NoticePhase, StartingNotice},
};
use sugarsubstitute_desktop::{HeadlessNotice, NoticeSurface};
use sugarsubstitute_execution::ExecutionClient;
use sugarsubstitute_splash::{
    AnimationFrame, Appearance, LOGICAL_SIZE, PreparationError, RenderError,
    SPLASH_CACHE_RESERVATION_BYTES, SplashPreparationResult, SplashRenderRequest, SplashRenderer,
    StartupStatus, Theme,
};
use sugarsubstitute_ui::{accent::AccentPalette, progress::ProgressFrame};

/// The launcher's notice copy; see the module documentation.
const LOCALE: &str = "en";

type Initialization = OwnedJob<Result<SplashRenderer, RenderError>, PreparationError>;
type Preparation = OwnedJob<SplashPreparationResult, PreparationError>;

/// What the launcher lends the notice for one turn.
pub(super) struct Resources<'a> {
    pub client: &'a ExecutionClient,
    pub scope: &'a Scope,
    pub jobs: &'a crate::job_sequence::JobSequence,
    /// The launch's own deadline; notice jobs never outlive it.
    pub until: Moment,
}

pub(super) struct Notice {
    policy: StartingNotice,
    /// Native wake at the notice moment while it is still waiting.
    wake: Option<Instant>,
    surface: Option<NoticeSurface>,
    initialization: Option<Initialization>,
    preparation: Option<Preparation>,
    /// The frame for the reported surface, until the pump takes it.
    frame: Option<PreparedFrame>,
    /// A frame was already prepared for the reported surface.
    prepared: bool,
    failed: bool,
}

impl Notice {
    /// A notice that may appear at the budget's notice moment.
    pub(super) fn new(budget: LauncherBudget) -> Self {
        Self {
            policy: StartingNotice::new(budget),
            wake: Instant::now().checked_add(LauncherBudget::NOTICE_DELAY),
            surface: None,
            initialization: None,
            preparation: None,
            frame: None,
            prepared: false,
            failed: false,
        }
    }

    /// Observe the clock and whether the launcher still owns startup.
    pub(super) fn observe(&mut self, now: Moment, owns_startup: bool) {
        if self.policy.observe(now, owns_startup) == Some(NoticeChange::Withdraw) {
            self.frame = None;
        }
        if self.policy.phase() != NoticePhase::Waiting {
            self.wake = None;
        }
    }

    /// Whether the notice is on screen now: the launcher then lets the pump build its loop.
    pub(super) fn shown(&self) -> bool {
        self.policy.phase() == NoticePhase::Shown && !self.failed
    }

    /// The native wake for the notice moment while it is still waiting.
    pub(super) const fn next_wake(&self) -> Option<Instant> {
        self.wake
    }

    /// Whether a notice job still runs; the launcher's executor cannot settle before it ends.
    pub(super) fn busy(&self) -> bool {
        self.initialization.is_some() || self.preparation.is_some()
    }

    /// Stop preparing frames; running jobs observe cancellation and end.
    pub(super) fn cancel(&self) {
        if let Some(job) = &self.initialization {
            job.cancel();
        }
        if let Some(job) = &self.preparation {
            job.cancel();
        }
    }

    /// The notice for the pump while it is shown; its frame is handed over once.
    pub(super) fn wanted(&mut self) -> Option<HeadlessNotice> {
        self.shown().then(|| HeadlessNotice {
            title: crate::APPLICATION_NAME,
            logical_size: [LOGICAL_SIZE; 2],
            frame: self.frame.take(),
        })
    }

    /// Prepare frames for the window the pump created.
    pub(super) fn surface(&mut self, surface: NoticeSurface) {
        self.surface = Some(surface);
        self.prepared = false;
        self.frame = None;
    }

    /// Consume finished notice jobs and, while shown, admit the next one. Closing only drains.
    pub(super) fn advance(&mut self, resources: &Resources<'_>, closing: bool) {
        if let Err(cause) = self.step(resources, closing) {
            tracing::warn!(operation = "launcher_notice", cause = %crate::failure_details::Details(cause.as_ref()),
                "The starting notice could not be prepared; startup continues without it");
            self.failed = true;
            self.frame = None;
            self.cancel();
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
            self.frame = Some(result.frame?);
            self.prepared = true;
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
        let request = request(surface)?;
        let bytes = request.reservation_bytes();
        self.preparation = Some(resources.client.submit_cpu(
            resources.scope,
            spec(resources, bytes)?,
            move |context| renderer.prepare(request, context.memory().clone()),
        )?);
        Ok(())
    }
}

/// The splash, as the launcher's notice, for `surface`.
fn request(surface: NoticeSurface) -> Result<SplashRenderRequest, Box<dyn Error>> {
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
            status: StartupStatus::Starting,
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
#[path = "notice_tests.rs"]
mod tests;
