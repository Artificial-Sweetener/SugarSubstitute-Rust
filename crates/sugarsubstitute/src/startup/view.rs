// SPDX-License-Identifier: GPL-3.0-or-later
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
use sugarsubstitute_ui::progress::ProgressFrame;
#[path = "view/disclosure.rs"]
mod disclosure;

/// Keep view-only disclosure and motion independent of native/connection authority.
pub(crate) struct StartupView {
    pub(crate) appearance: Appearance,
    pub(crate) animation: PaperFlip,
    pub(crate) elapsed: Duration,
    reduced_motion: bool,
    pressed: Option<Control>,
    messages: &'static Messages,
    controls: [AccessibleControl; 3],
    retry_requested: bool,
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
    pub(crate) fn new(presentation: Presentation) -> Self {
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
                expanded: false,
                reveal: 0.0,
                hovered: None,
                pressed: None,
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
            retry_requested: false,
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
                AccessibleControl {
                    enabled: true,
                    id: 3,
                    label: messages.retry,
                    bounds: ControlBounds { x: 24.0, y: 486.0, width: 510.0, height: 48.0 },
                    close: false,
                    visible: false,
                },
            ],
        }
    }
    /// Describe the existing console inner rectangle only after matching frame publication.
    /// Leave the settled Retry peer's original rectangle available above this native child.
    pub(crate) fn console<'a>(
        &self,
        text: Option<&'a dyn sugarsubstitute_desktop::ConsoleText>,
    ) -> sugarsubstitute_desktop::ConsoleView<'a> {
        let top = 545.0 - 174.0 * self.published_reveal;
        let bottom = if self.controls[2].visible { 480.0 } else { 533.0 };
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
    /// Consume only an actual pointer/accessibility activation, never a rendered fact.
    pub(crate) fn take_retry(&mut self) -> bool {
        std::mem::take(&mut self.retry_requested)
    }
    /// Project the sole application phase into native copy and accessible affordances.
    pub(crate) fn phase(&mut self, phase: StartupPhase) {
        if self.appearance.status == sugarsubstitute_splash::StartupStatus::RetryReady
            && phase == StartupPhase::SplashFirstFrame
        {
            self.wait_started = self.elapsed;
            self.reset_wait_on_sample = true;
            self.progress = StartupProgress::default();
            self.output_revision = None;
        }
        self.progress.sample(phase, self.elapsed, self.reduced_motion);
        self.appearance.operation = self.progress.operation();
        self.controls[2].visible = phase == StartupPhase::Stopped;
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
    }
    /// Sample a real frame and schedule only the next text transition, including reduced motion.
    pub(crate) fn sample(&mut self, phase: StartupPhase, elapsed: Duration, now: Instant) {
        self.elapsed = elapsed;
        self.appearance.reveal = self.disclosure.sample(elapsed);
        self.phase(phase);
        if std::mem::take(&mut self.reset_wait_on_sample) {
            self.wait_started = elapsed;
            self.phase(phase);
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
        if self.appearance.status == sugarsubstitute_splash::StartupStatus::RetryReady
            && (24.0..534.0).contains(&pointer.x)
            && (486.0..534.0).contains(&pointer.y)
        {
            return Some(Control::Retry);
        }
        if !(0.0..32.0).contains(&pointer.y) {
            return None;
        }
        if (512.0..558.0).contains(&pointer.x) {
            Some(Control::Close)
        } else if (466.0..512.0).contains(&pointer.x) {
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
                if self.pressed == Some(Control::Retry) && control == self.pressed {
                    self.retry_requested = true;
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
    /// Keep every native identity preallocated; phase changes only Retry visibility and eligibility.
    pub(crate) fn controls(&self) -> &[AccessibleControl] {
        &self.controls
    }
    /// Activate only the non-close disclosure control; desktop owns native close intent.
    pub(crate) fn activate(&mut self, id: u16) -> bool {
        if id == 3 && self.appearance.status == sugarsubstitute_splash::StartupStatus::RetryReady {
            self.retry_requested = true;
            return true;
        }
        if id != 1 {
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

#[cfg(test)]
#[path = "view/interaction_tests.rs"]
mod interaction_tests;
#[cfg(test)]
#[path = "view/output_tests.rs"]
mod output_tests;
#[cfg(test)]
#[path = "view/tests.rs"]
mod tests;
