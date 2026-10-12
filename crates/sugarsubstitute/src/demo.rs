// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Drive an explicit demonstration without claiming real application startup progress.

use crate::preparation::PreparationDriver;
use sugarsubstitute_execution::{ExecutionOwner, MonotonicClock};

#[path = "demo_lifetime.rs"]
mod lifetime;
use std::{error::Error, num::NonZeroUsize, sync::Arc, time::Duration};
use sugarsubstitute_contracts::{
    execution::{Clock, CompletionWake},
    presentation::{FrameShape, PreparedFrame},
};
use sugarsubstitute_desktop::{
    AccessibleControl, ControlBounds, HitTarget, KeyAction, Pointer, PointerAction, WindowContent,
};
use sugarsubstitute_splash::{Appearance, Control, PaperFlip, ProgressFrame};

/// Keep splash-specific interaction semantics outside the shared desktop adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DemoTarget {
    Caption,
    Close,
    Toggle,
    Mascot,
    Console,
}

/// Compose the presentation driver used until real startup supplies semantic events.
pub(crate) struct SplashDemo {
    preparation: Option<PreparationDriver>,
    locale: String,
    execution: Option<ExecutionOwner>,
    clock: Arc<dyn Clock>,
    visible: bool,
    closing: bool,
    shutdown_deadline_reported: bool,
    animation: PaperFlip,
    appearance: Appearance,
    elapsed: Duration,
    pressed_target: Option<DemoTarget>,
    reduced_motion: bool,
    controls: [AccessibleControl; 2],
    messages: &'static sugarsubstitute_contracts::localization::Messages,
}

impl SplashDemo {
    /// Start a seeded demo with blank output and independently animated progress.
    pub(crate) fn new(
        locale: &str,
        appearance: Appearance,
        reduced_motion: bool,
    ) -> Result<Self, Box<dyn Error>> {
        let mut animation = PaperFlip::new(0x0053_5547_4152);
        animation.set_reduced_motion(reduced_motion);
        let messages = sugarsubstitute_contracts::localization::messages(locale);
        Ok(Self {
            preparation: None,
            locale: locale.to_owned(),
            execution: None,
            clock: MonotonicClock::new(),
            visible: false,
            closing: false,
            shutdown_deadline_reported: false,
            animation,
            appearance,
            elapsed: Duration::ZERO,
            pressed_target: None,
            reduced_motion,
            messages,
            controls: [
                AccessibleControl {
                    id: 1,
                    label: if appearance.expanded {
                        messages.hide_output
                    } else {
                        messages.show_output
                    },
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
        })
    }
}

impl SplashDemo {
    /// Change requested disclosure state before invalidating preparation.
    fn toggle_output(&mut self) {
        self.appearance.expanded = !self.appearance.expanded;
    }

    /// Publish the control snapshot with its matching current-state pixels.
    fn publish_controls(&mut self) {
        self.controls[0].label = if self.appearance.expanded {
            self.messages.hide_output
        } else {
            self.messages.show_output
        };
    }

    /// Classify the splash's private controls and passive regions.
    fn classify(&self, pointer: Pointer) -> DemoTarget {
        if pointer.y >= 0.0 && pointer.y < 32.0 {
            if (512.0..558.0).contains(&pointer.x) {
                return DemoTarget::Close;
            }
            if (466.0..512.0).contains(&pointer.x) {
                return DemoTarget::Toggle;
            }
            return DemoTarget::Caption;
        }
        if self.appearance.expanded
            && (24.0..534.0).contains(&pointer.x)
            && (356.0..534.0).contains(&pointer.y)
        {
            return DemoTarget::Console;
        }
        let mascot = self.appearance.mascot();
        if (mascot.x..mascot.x + mascot.width).contains(&pointer.x)
            && (mascot.y..mascot.y + mascot.height).contains(&pointer.y)
        {
            return DemoTarget::Mascot;
        }
        DemoTarget::Caption
    }
}

impl WindowContent for SplashDemo {
    /// Start one execution lifetime after the desktop establishes its native wake.
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>> {
        self.start_execution(wake)
    }

    /// Sample lightweight view state and submit governed preparation without rasterizing.
    fn request_frame(
        &mut self,
        shape: FrameShape,
        elapsed: Duration,
    ) -> Result<(), Box<dyn Error>> {
        self.elapsed = elapsed;
        self.animation.tick(elapsed);
        let cycle = if self.reduced_motion { 0.0 } else { elapsed.as_secs_f32() % 12.0 };
        let completed = (cycle / 10.0).min(1.0);
        let activity = if self.reduced_motion || completed == 1.0 {
            None
        } else {
            Some((elapsed.as_secs_f32() % 1.8) / 1.8)
        };
        let feedback = ProgressFrame { completed, activity };
        self.preparation.as_mut().ok_or("Demo execution has not started")?.request(
            shape,
            self.appearance,
            self.animation.frame(elapsed),
            feedback,
        )
    }

    /// Observe bounded terminal work and publish only matching current-state results.
    fn poll_frame(
        &mut self,
        maximum: NonZeroUsize,
    ) -> Result<Option<PreparedFrame>, Box<dyn Error>> {
        let frame = self.consume_preparation(maximum)?;
        if frame.is_some() {
            self.publish_controls();
        }
        Ok(frame)
    }

    /// Invalidate visibility before subsequent raster admission or publication.
    fn set_visible(&mut self, visible: bool) -> Result<(), Box<dyn Error>> {
        self.visible = visible && !self.closing;
        if let Some(preparation) = &mut self.preparation {
            preparation.set_visible(self.visible)?;
        }
        Ok(())
    }

    /// Close admission and request actual cancellation without waiting on this callback.
    fn request_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        self.retire_execution()
    }

    /// Keep terminal and worker settlement observable while the native loop remains alive.
    fn poll_shutdown(&mut self) -> Result<bool, Box<dyn Error>> {
        self.poll_execution_retirement()
    }

    /// Release retained cache resources and join settled workers after native teardown.
    fn finish_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        self.finish_execution_retirement()
    }

    /// Reserve native drag and close behavior without exposing view-specific targets.
    fn hit_test(&self, pointer: Pointer) -> HitTarget {
        match self.classify(pointer) {
            DemoTarget::Caption => HitTarget::Caption,
            DemoTarget::Close => HitTarget::Close,
            _ => HitTarget::Client,
        }
    }

    /// Apply view-only disclosure, hover and click-to-flip behavior.
    fn pointer(&mut self, action: PointerAction, pointer: Pointer) -> bool {
        let before = (self.appearance.hovered, self.appearance.pressed, self.appearance.expanded);
        let mut animation_changed = false;
        let target = self.classify(pointer);
        let control = match target {
            DemoTarget::Toggle => Some(Control::ConsoleToggle),
            DemoTarget::Close => Some(Control::Close),
            _ => None,
        };
        match action {
            PointerAction::Move => self.appearance.hovered = control,
            PointerAction::Leave => {
                self.appearance.hovered = None;
                self.appearance.pressed = None;
                self.pressed_target = None;
            }
            PointerAction::Press => {
                self.appearance.pressed = control;
                self.pressed_target = Some(target);
                if target == DemoTarget::Mascot {
                    animation_changed = self.animation.trigger_flip(self.elapsed);
                }
            }
            PointerAction::Release => {
                if self.pressed_target == Some(target) && target == DemoTarget::Toggle {
                    self.toggle_output();
                }
                self.appearance.pressed = None;
                self.pressed_target = None;
            }
        }
        let changed = animation_changed
            || before
                != (self.appearance.hovered, self.appearance.pressed, self.appearance.expanded);
        if changed && let Some(preparation) = &mut self.preparation {
            preparation.invalidate();
        }
        changed
    }

    /// Route the adapter's keyboard disclosure intent to the same view state.
    fn key(&mut self, action: KeyAction) -> bool {
        match action {
            KeyAction::Tab => self.toggle_output(),
        }
        if let Some(preparation) = &mut self.preparation {
            preparation.invalidate();
        }
        true
    }

    /// Freeze both scripted progress and mascot motion when reduced motion is selected.
    fn animating(&self) -> bool {
        !self.reduced_motion || !self.preparation.as_ref().is_some_and(PreparationDriver::enriched)
    }

    /// Expose only the two existing caption controls through stable native identities.
    fn accessible_controls(&self) -> &[AccessibleControl] {
        &self.controls
    }

    /// Apply native assistive-technology disclosure through the pointer/keyboard state owner.
    fn activate(&mut self, id: u16) -> bool {
        if id == 1 {
            self.toggle_output();
            if let Some(preparation) = &mut self.preparation {
                preparation.invalidate();
            }
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
#[path = "demo_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "demo_test_support.rs"]
mod test_support;
