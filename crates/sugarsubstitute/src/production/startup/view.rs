// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Present acknowledged startup progress and localized splash interaction.

use std::time::Duration;
use sugarsubstitute_application::startup::StartupPhase;
use sugarsubstitute_contracts::{
    configuration::{Presentation, Theme as RequestedTheme},
    localization::Messages,
};
use sugarsubstitute_desktop::{
    AccessibleControl, ControlBounds, HitTarget, KeyAction, Pointer, PointerAction,
};
use sugarsubstitute_splash::{Appearance, Control, PaperFlip, ProgressFrame, Theme};

/// Keep view-only disclosure and motion independent of native/connection authority.
pub(in crate::production) struct StartupView {
    pub(in crate::production) appearance: Appearance,
    pub(in crate::production) animation: PaperFlip,
    pub(in crate::production) elapsed: Duration,
    reduced_motion: bool,
    pressed: Option<Control>,
    messages: &'static Messages,
    controls: [AccessibleControl; 3],
    retry_requested: bool,
    console_expanded: bool,
}
impl StartupView {
    /// Resolve owned text through the sole locale registry; allocate no raster resources.
    pub(in crate::production) fn new(presentation: Presentation) -> Self {
        let messages =
            sugarsubstitute_contracts::localization::messages(presentation.locale().as_str());
        let mut animation = PaperFlip::new(0x0053_5547_4152);
        animation.set_reduced_motion(presentation.reduced_motion());
        Self {
            appearance: Appearance {
                status: sugarsubstitute_splash::StartupStatus::Loading,
                theme: match presentation.theme() {
                    RequestedTheme::Dark => Theme::Dark,
                    RequestedTheme::Light => Theme::Light,
                },
                expanded: false,
                hovered: None,
                pressed: None,
            },
            animation,
            elapsed: Duration::ZERO,
            reduced_motion: presentation.reduced_motion(),
            pressed: None,
            retry_requested: false,
            console_expanded: false,
            messages,
            controls: [
                AccessibleControl {
                    id: 1,
                    label: messages.show_output,
                    bounds: ControlBounds { x: 466.0, y: 0.0, width: 46.0, height: 32.0 },
                    close: false,
                    visible: true,
                },
                AccessibleControl {
                    id: 2,
                    label: messages.cancel,
                    bounds: ControlBounds { x: 512.0, y: 0.0, width: 46.0, height: 32.0 },
                    close: true,
                    visible: true,
                },
                AccessibleControl {
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
    pub(in crate::production) fn console<'a>(
        &self,
        text: Option<&'a dyn sugarsubstitute_desktop::ConsoleText>,
    ) -> sugarsubstitute_desktop::ConsoleView<'a> {
        sugarsubstitute_desktop::ConsoleView {
            label: self.messages.output,
            bounds: ControlBounds {
                x: 25.0,
                y: 371.0,
                width: 508.0,
                height: if self.controls[2].visible { 109.0 } else { 162.0 },
            },
            visible: self.console_expanded,
            dark: self.appearance.theme == Theme::Dark,
            text,
        }
    }
    /// Two acknowledged milestones: backend ready, then actual frontend ready.
    /// Elapsed time drives only activity/motion and never advances completed work.
    pub(in crate::production) fn progress(&self, phase: StartupPhase) -> ProgressFrame {
        let completed = match phase {
            StartupPhase::ServerReady | StartupPhase::WebviewLoading => 0.5,
            StartupPhase::FrontendReady | StartupPhase::RevealingMain | StartupPhase::Running => {
                1.0
            }
            _ => 0.0,
        };
        let active = matches!(
            phase,
            StartupPhase::PreparingConnection
                | StartupPhase::ServerReady
                | StartupPhase::WebviewLoading
        );
        ProgressFrame {
            completed,
            activity: (active && !self.reduced_motion)
                .then(|| (self.elapsed.as_secs_f32() % 1.8) / 1.8),
        }
    }
    /// Match the accessible disclosure label to the last successfully published frame.
    pub(in crate::production) fn publish_controls(&mut self) {
        self.console_expanded = self.appearance.expanded;
        self.controls[0].label = if self.appearance.expanded {
            self.messages.hide_output
        } else {
            self.messages.show_output
        };
    }
    /// Consume only an actual pointer/accessibility activation, never a rendered fact.
    pub(in crate::production) fn take_retry(&mut self) -> bool {
        std::mem::take(&mut self.retry_requested)
    }
    /// Project the sole application phase into native copy and accessible affordances.
    pub(in crate::production) fn phase(&mut self, phase: StartupPhase) {
        self.controls[2].visible = phase == StartupPhase::Stopped;
        self.appearance.status = match phase {
            StartupPhase::Failed | StartupPhase::Cancelling => {
                sugarsubstitute_splash::StartupStatus::Settling
            }
            StartupPhase::Stopped => sugarsubstitute_splash::StartupStatus::RetryReady,
            _ => sugarsubstitute_splash::StartupStatus::Loading,
        };
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
    /// Reserve native caption dragging/close while all other content remains client-owned.
    pub(in crate::production) fn hit_test(&self, pointer: Pointer) -> HitTarget {
        match self.control(pointer) {
            Some(Control::Close) => HitTarget::Close,
            Some(_) => HitTarget::Client,
            None if (0.0..32.0).contains(&pointer.y) => HitTarget::Caption,
            None => HitTarget::Client,
        }
    }
    /// Apply disclosure and hover using the same geometry as painted/native controls.
    pub(in crate::production) fn pointer(
        &mut self,
        action: PointerAction,
        pointer: Pointer,
    ) -> bool {
        let before = (self.appearance.expanded, self.appearance.hovered, self.appearance.pressed);
        let control = self.control(pointer);
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
            }
            PointerAction::Release => {
                if self.pressed == Some(Control::ConsoleToggle) && control == self.pressed {
                    self.appearance.expanded = !self.appearance.expanded;
                }
                if self.pressed == Some(Control::Retry) && control == self.pressed {
                    self.retry_requested = true;
                }
                self.pressed = None;
                self.appearance.pressed = None;
            }
        }
        before != (self.appearance.expanded, self.appearance.hovered, self.appearance.pressed)
    }
    /// Focus navigation never activates disclosure or changes startup state.
    pub(in crate::production) fn key(&mut self, _action: KeyAction) -> bool {
        false
    }
    /// Keep every native identity preallocated; phase changes only Retry visibility and eligibility.
    pub(in crate::production) fn controls(&self) -> &[AccessibleControl] {
        &self.controls
    }
    /// Activate only the non-close disclosure control; desktop owns native close intent.
    pub(in crate::production) fn activate(&mut self, id: u16) -> bool {
        if id == 3 && self.appearance.status == sugarsubstitute_splash::StartupStatus::RetryReady {
            self.retry_requested = true;
            return true;
        }
        if id != 1 {
            return false;
        }
        self.appearance.expanded = !self.appearance.expanded;
        true
    }
    /// Static reduced-motion presentation requires only the renderer's initial enrichment.
    pub(in crate::production) const fn motion(&self) -> bool {
        !self.reduced_motion
    }
}

#[cfg(test)]
#[path = "view/tests.rs"]
mod tests;
