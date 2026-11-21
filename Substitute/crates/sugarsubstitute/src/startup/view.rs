// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Present acknowledged startup progress and localized splash interaction.

use std::time::{Duration, Instant};
use sugarsubstitute_application::startup::{StartupPhase, StartupProgress};
use sugarsubstitute_contracts::{
    configuration::{Presentation, Theme as RequestedTheme},
    localization::Messages,
};
use sugarsubstitute_desktop::{
    AccessibleControl, ControlBounds, HitTarget, KeyAction, Pointer, PointerAction,
};
use sugarsubstitute_splash::{Appearance, Control, PaperFlip, Theme};
use sugarsubstitute_ui::{accent::AccentPalette, progress::ProgressFrame};
#[path = "view/disclosure.rs"]
mod disclosure;

/// Tries in a row the splash makes by itself before it asks the person instead.
const MAXIMUM_AUTOMATIC_RETRIES: u32 = 5;

/// Keep view-only disclosure and motion independent of native/connection authority.
pub(crate) struct StartupView {
    pub(crate) appearance: Appearance,
    pub(crate) animation: PaperFlip,
    pub(crate) elapsed: Duration,
    reduced_motion: bool,
    pressed: Option<Control>,
    messages: &'static Messages,
    controls: [AccessibleControl; 2],
    /// A settled failed start is due to be tried again by itself.
    retry_due: bool,
    /// Tries in a row that failed since the launch or the person's last Retry.
    retries: u32,
    /// A retry replaced the launch's first attempt, so its update has already been applied.
    retried: bool,
    published_reveal: f64,
    disclosure: disclosure::Disclosure,
    wait_started: Duration,
    feedback_deadline: Option<Instant>,
    reset_wait_on_sample: bool,
    progress: StartupProgress,
    output_revision: Option<(sugarsubstitute_contracts::lifecycle::ResourceKey, u64)>,
}
impl StartupView {
    /// Resolve owned text through the sole locale registry; allocate no raster resources.
    /// `accent` is the palette resolved from the current operating-system accent.
    pub(crate) fn new(presentation: Presentation, accent: AccentPalette) -> Self {
        let messages =
            sugarsubstitute_contracts::localization::messages(presentation.locale().as_str());
        let mut animation = PaperFlip::new(0x0053_5547_4152);
        animation.set_reduced_motion(presentation.reduced_motion());
        Self {
            appearance: Appearance {
                status: sugarsubstitute_splash::StartupStatus::Loading,
                operation: Default::default(),
                theme: match presentation.theme() {
                    RequestedTheme::Dark => Theme::Dark,
                    RequestedTheme::Light => Theme::Light,
                },
                accent,
                expanded: false,
                reveal: 0.0,
                hovered: None,
                pressed: None,
                controls: true,
                output: true,
                failure: None,
                retrying: false,
            },
            animation,
            elapsed: Duration::ZERO,
            wait_started: Duration::ZERO,
            feedback_deadline: None,
            reset_wait_on_sample: false,
            progress: StartupProgress::default(),
            output_revision: None,
            reduced_motion: presentation.reduced_motion(),
            pressed: None,
            retry_due: false,
            retries: 0,
            retried: false,
            published_reveal: 0.0,
            disclosure: disclosure::Disclosure::new(),
            messages,
            controls: [
                AccessibleControl {
                    enabled: true,
                    id: 1,
                    label: messages.show_output,
                    bounds: ControlBounds { x: 466.0, y: 0.0, width: 46.0, height: 32.0 },
                    close: false,
                    visible: true,
                },
                AccessibleControl {
                    enabled: true,
                    id: 2,
                    label: messages.cancel,
                    bounds: ControlBounds { x: 512.0, y: 0.0, width: 46.0, height: 32.0 },
                    close: true,
                    visible: true,
                },
            ],
        }
    }
    /// Offer the output disclosure only when this start writes ComfyUI output to the
    /// application: a ComfyUI the application attaches to writes none.
    pub(crate) fn set_output_available(&mut self, available: bool) {
        self.appearance.output = available;
        self.controls[0].visible = available;
    }
    /// Whether the failed start shown needs the person: a failure retrying cannot fix, or one
    /// that five tries in a row did not fix. A dialog explains it with Retry and Close.
    pub(crate) fn asks_about_failure(&self) -> bool {
        self.appearance.status.offers_retry() && !self.retries_by_itself()
    }
    /// Repaint with a changed operating-system accent; returns whether the palette changed.
    pub(crate) fn set_accent(&mut self, accent: AccentPalette) -> bool {
        let changed = self.appearance.accent != accent;
        self.appearance.accent = accent;
        changed
    }
    /// Describe the existing console inner rectangle only after matching frame publication.
    pub(crate) fn console<'a>(
        &self,
        text: Option<&'a dyn sugarsubstitute_desktop::ConsoleText>,
    ) -> sugarsubstitute_desktop::ConsoleView<'a> {
        let top = 545.0 - 174.0 * self.published_reveal;
        let bottom = 533.0;
        sugarsubstitute_desktop::ConsoleView {
            label: self.messages.output,
            bounds: ControlBounds {
                x: 25.0,
                y: top.min(bottom - 1.0),
                width: 508.0,
                height: (bottom - top).max(1.0),
            },
            visible: top < bottom,
            dark: self.appearance.theme == Theme::Dark,
            text,
        }
    }
    /// Render the application's one whole-startup estimate and bounded activity sweep.
    pub(crate) fn progress(&self) -> ProgressFrame {
        ProgressFrame {
            completed: self.progress.completed(),
            activity: (!self.reduced_motion)
                .then(|| self.progress.activity_phase(self.elapsed))
                .flatten(),
        }
    }
    /// Consume each retained output revision once, including with reduced motion.
    /// The adapter recognizes bounded transcript lines; readiness still comes from policy.
    pub(crate) fn output(
        &mut self,
        source: Option<&sugarsubstitute_application::transcript::TranscriptSnapshot>,
    ) -> bool {
        let Some(source) = source else { return false };
        let identity = (source.resource(), source.revision());
        if self.output_revision == Some(identity) {
            return false;
        }
        self.output_revision = Some(identity);
        self.progress.output(None, self.elapsed);
        for record in source.records() {
            if let Some(operation) =
                sugarsubstitute_comfy::startup_feedback::operation(record.text())
            {
                self.progress.output(Some(operation), self.elapsed);
            }
        }
        self.appearance.operation = self.progress.operation();
        true
    }
    /// Match the accessible disclosure label to the last successfully published frame.
    pub(crate) fn publish_controls(&mut self, appearance: Appearance) {
        self.published_reveal = appearance.reveal;
        self.controls[0].label =
            if appearance.expanded { self.messages.hide_output } else { self.messages.show_output };
    }
    /// Consume the retry that falls due once a failed start has settled. The splash never asks
    /// anyone to retry: it names the failure and tries again by itself as soon as the failed
    /// attempt has settled, up to [`MAXIMUM_AUTOMATIC_RETRIES`] tries in a row.
    pub(crate) fn take_retry(&mut self) -> bool {
        if !std::mem::take(&mut self.retry_due) {
            return false;
        }
        self.retries = self.retries.saturating_add(1);
        true
    }
    /// Whether the splash still tries the failed start again by itself: only a failure that
    /// retrying can fix, and only until five tries in a row have failed.
    fn retries_by_itself(&self) -> bool {
        self.appearance.retries_by_itself() && self.retries < MAXIMUM_AUTOMATIC_RETRIES
    }
    /// Start counting tries again, after the person chose Retry in the dialog.
    pub(crate) fn reset_retries(&mut self) {
        self.retries = 0;
    }
    /// Schedule the next retry while the status line names a failed start the splash retries by
    /// itself, and drop it otherwise; a failure that needs the person is asked in a dialog.
    fn schedule_retry(&mut self) {
        self.appearance.retrying = self.retries_by_itself();
        self.retry_due = self.appearance.retrying;
    }
    /// Project the sole application phase, with the reason its start failed, into native copy
    /// and accessible affordances. The two arrive together so a settled failure is never judged
    /// for an automatic retry before its reason is known.
    pub(crate) fn observe(
        &mut self,
        phase: StartupPhase,
        failure: Option<sugarsubstitute_contracts::startup::StartupFailureCode>,
    ) {
        self.appearance.failure = failure;
        if self.appearance.status.offers_retry() && phase == StartupPhase::SplashFirstFrame {
            self.wait_started = self.elapsed;
            self.reset_wait_on_sample = true;
            self.progress = StartupProgress::default();
            self.output_revision = None;
        }
        self.progress.sample(phase, self.elapsed, self.reduced_motion);
        self.appearance.operation = self.progress.operation();
        self.appearance.status = match phase {
            StartupPhase::Failed | StartupPhase::Cancelling => {
                sugarsubstitute_splash::StartupStatus::Settling
            }
            StartupPhase::Stopped => sugarsubstitute_splash::StartupStatus::RetryReady,
            _ => match self.elapsed.saturating_sub(self.wait_started).as_secs() {
                300.. => sugarsubstitute_splash::StartupStatus::LoadingExtended,
                120.. => sugarsubstitute_splash::StartupStatus::LoadingLong,
                _ => sugarsubstitute_splash::StartupStatus::Loading,
            },
        };
        self.schedule_retry();
    }
    /// A retry replaced the launch's first attempt.
    pub(crate) fn begin_retry(&mut self) {
        self.retried = true;
        self.observe(StartupPhase::SplashFirstFrame, None);
    }
    /// Whether a retry replaced the launch's first attempt; the version this launch updated to
    /// is then already running, so its "Updating to version" line is not shown again.
    pub(crate) const fn retried(&self) -> bool {
        self.retried
    }
    /// Show `status` in place of the phase's own line; a retry is scheduled only while the shown
    /// line names a failed start the splash retries by itself.
    pub(crate) fn show_status(&mut self, status: sugarsubstitute_splash::StartupStatus) {
        self.appearance.status = status;
        self.schedule_retry();
    }
    /// Sample a real frame and schedule only the next text transition, including reduced motion.
    pub(crate) fn sample(
        &mut self,
        phase: StartupPhase,
        failure: Option<sugarsubstitute_contracts::startup::StartupFailureCode>,
        elapsed: Duration,
        now: Instant,
    ) {
        self.elapsed = elapsed;
        self.appearance.reveal = self.disclosure.sample(elapsed);
        self.observe(phase, failure);
        if std::mem::take(&mut self.reset_wait_on_sample) {
            self.wait_started = elapsed;
            self.observe(phase, failure);
        }
        let threshold = match self.appearance.status {
            sugarsubstitute_splash::StartupStatus::Loading => Some(Duration::from_secs(120)),
            sugarsubstitute_splash::StartupStatus::LoadingLong => Some(Duration::from_secs(300)),
            _ => None,
        };
        self.feedback_deadline = threshold.and_then(|threshold| {
            now.checked_add(threshold.saturating_sub(elapsed.saturating_sub(self.wait_started)))
        });
    }
    /// Wake the native event loop for static text without scheduling continuous motion.
    pub(crate) const fn feedback_deadline(&self) -> Option<Instant> {
        self.feedback_deadline
    }
    /// Request one fresh frame when a waiting-message transition becomes due.
    pub(crate) fn feedback_due(&self, now: Instant) -> bool {
        self.feedback_deadline.is_some_and(|deadline| now >= deadline)
    }
    fn control(&self, pointer: Pointer) -> Option<Control> {
        if !(0.0..32.0).contains(&pointer.y) {
            return None;
        }
        if (512.0..558.0).contains(&pointer.x) {
            Some(Control::Close)
        } else if self.appearance.output && (466.0..512.0).contains(&pointer.x) {
            Some(Control::ConsoleToggle)
        } else {
            None
        }
    }
    /// Drag every noninteractive region while retaining published console input and live controls.
    pub(crate) fn hit_test(&self, pointer: Pointer) -> HitTarget {
        let console = self.console(None);
        let bounds = console.bounds;
        let over_console = console.visible
            && (bounds.x..bounds.x + bounds.width).contains(&pointer.x)
            && (bounds.y..bounds.y + bounds.height).contains(&pointer.y);
        match self.control(pointer) {
            Some(Control::Close) => HitTarget::Close,
            Some(_) => HitTarget::Client,
            None if !over_console
                && (0.0..558.0).contains(&pointer.x)
                && (0.0..558.0).contains(&pointer.y) =>
            {
                HitTarget::Caption
            }
            None => HitTarget::Client,
        }
    }
    /// Apply disclosure and hover using the same geometry as painted/native controls.
    pub(crate) fn pointer(&mut self, action: PointerAction, pointer: Pointer) -> bool {
        let before = (self.appearance.expanded, self.appearance.hovered, self.appearance.pressed);
        let control = self.control(pointer);
        let mut flipped = false;
        match action {
            PointerAction::Move => self.appearance.hovered = control,
            PointerAction::Leave => {
                self.appearance.hovered = None;
                self.appearance.pressed = None;
                self.pressed = None;
            }
            PointerAction::Press => {
                self.pressed = control;
                self.appearance.pressed = control;
                let mascot =
                    Appearance { reveal: self.published_reveal, ..self.appearance }.mascot();
                if control.is_none()
                    && self.hit_test(pointer) == HitTarget::Caption
                    && (mascot.x..mascot.x + mascot.width).contains(&pointer.x)
                    && (mascot.y..mascot.y + mascot.height).contains(&pointer.y)
                {
                    flipped = self.animation.trigger_flip(self.elapsed);
                }
            }
            PointerAction::Release => {
                if self.pressed == Some(Control::ConsoleToggle) && control == self.pressed {
                    self.toggle_console();
                }
                self.pressed = None;
                self.appearance.pressed = None;
            }
        }
        flipped
            || before
                != (self.appearance.expanded, self.appearance.hovered, self.appearance.pressed)
    }
    /// Focus navigation never activates disclosure or changes startup state.
    pub(crate) fn key(&mut self, _action: KeyAction) -> bool {
        false
    }
    /// Keep every native identity preallocated; attach mode hides only the output disclosure.
    pub(crate) fn controls(&self) -> &[AccessibleControl] {
        &self.controls
    }
    /// Activate only the non-close disclosure control; desktop owns native close intent.
    pub(crate) fn activate(&mut self, id: u16) -> bool {
        if id != 1 || !self.appearance.output {
            return false;
        }
        self.toggle_console();
        true
    }
    /// Use one transition for pointer and native accessible activation.
    fn toggle_console(&mut self) {
        self.appearance.expanded = self.disclosure.toggle(self.elapsed, self.reduced_motion);
        self.appearance.reveal = self.disclosure.sample(self.elapsed);
    }
    /// Static reduced-motion presentation requires only the renderer's initial enrichment.
    pub(crate) const fn motion(&self) -> bool {
        !self.reduced_motion
    }
}

/// The reason `session`'s start failed, as the splash names it.
pub(crate) fn failure_code(
    session: &sugarsubstitute_application::startup::StartupSession,
) -> Option<sugarsubstitute_contracts::startup::StartupFailureCode> {
    session.failure().and_then(sugarsubstitute_application::startup::SessionFailure::startup_code)
}

#[cfg(test)]
#[path = "view/interaction_tests.rs"]
mod interaction_tests;
#[cfg(test)]
#[path = "view/output_tests.rs"]
mod output_tests;
#[cfg(test)]
#[path = "view/tests.rs"]
mod tests;
