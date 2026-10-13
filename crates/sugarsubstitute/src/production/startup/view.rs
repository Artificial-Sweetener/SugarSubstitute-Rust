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
pub(super) struct StartupView {
    pub(super) appearance: Appearance,
    pub(super) animation: PaperFlip,
    pub(super) elapsed: Duration,
    reduced_motion: bool,
    pressed: Option<Control>,
    messages: &'static Messages,
    controls: [AccessibleControl; 2],
}
impl StartupView {
    /// Resolve owned text through the sole locale registry; allocate no raster resources.
    pub(super) fn new(presentation: Presentation) -> Self {
        let messages =
            sugarsubstitute_contracts::localization::messages(presentation.locale().as_str());
        let mut animation = PaperFlip::new(0x0053_5547_4152);
        animation.set_reduced_motion(presentation.reduced_motion());
        Self {
            appearance: Appearance {
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
            messages,
            controls: [
                AccessibleControl {
                    id: 1,
                    label: messages.show_output,
                    bounds: ControlBounds { x: 466.0, y: 0.0, width: 46.0, height: 32.0 },
                    close: false,
                },
                AccessibleControl {
                    id: 2,
                    label: messages.cancel,
                    bounds: ControlBounds { x: 512.0, y: 0.0, width: 46.0, height: 32.0 },
                    close: true,
                },
            ],
        }
    }
    /// Two acknowledged milestones: backend ready, then actual frontend ready.
    /// Elapsed time drives only activity/motion and never advances completed work.
    pub(super) fn progress(&self, phase: StartupPhase) -> ProgressFrame {
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
    pub(super) fn publish_controls(&mut self) {
        self.controls[0].label = if self.appearance.expanded {
            self.messages.hide_output
        } else {
            self.messages.show_output
        };
    }
    fn control(&self, pointer: Pointer) -> Option<Control> {
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
    pub(super) fn hit_test(&self, pointer: Pointer) -> HitTarget {
        match self.control(pointer) {
            Some(Control::Close) => HitTarget::Close,
            Some(_) => HitTarget::Client,
            None if (0.0..32.0).contains(&pointer.y) => HitTarget::Caption,
            None => HitTarget::Client,
        }
    }
    /// Apply disclosure and hover using the same geometry as painted/native controls.
    pub(super) fn pointer(&mut self, action: PointerAction, pointer: Pointer) -> bool {
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
                self.pressed = None;
                self.appearance.pressed = None;
            }
        }
        before != (self.appearance.expanded, self.appearance.hovered, self.appearance.pressed)
    }
    /// Route keyboard disclosure to the same presentation-only state.
    pub(super) fn key(&mut self, action: KeyAction) -> bool {
        match action {
            KeyAction::Tab => self.appearance.expanded = !self.appearance.expanded,
        }
        true
    }
    /// Expose stable native accessibility peers for the exact painted caption controls.
    pub(super) fn controls(&self) -> &[AccessibleControl] {
        &self.controls
    }
    /// Activate only the non-close disclosure control; desktop owns native close intent.
    pub(super) fn activate(&mut self, id: u16) -> bool {
        if id != 1 {
            return false;
        }
        self.appearance.expanded = !self.appearance.expanded;
        true
    }
    /// Static reduced-motion presentation requires only the renderer's initial enrichment.
    pub(super) const fn motion(&self) -> bool {
        !self.reduced_motion
    }
}

#[cfg(test)]
#[path = "view/tests.rs"]
mod tests;
