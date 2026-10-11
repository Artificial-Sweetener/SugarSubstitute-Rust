// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Drive an explicit demonstration without claiming real application startup progress.

use std::{error::Error, time::Duration};
use sugarsubstitute_desktop::{Frame, HitTarget, KeyAction, Pointer, PointerAction, WindowContent};
use sugarsubstitute_splash::{Appearance, Control, PaperFlip, ProgressFrame, SplashRenderer};

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
    renderer: SplashRenderer,
    animation: PaperFlip,
    appearance: Appearance,
    elapsed: Duration,
    pressed_target: Option<DemoTarget>,
    reduced_motion: bool,
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
        Ok(Self {
            renderer: SplashRenderer::new(locale)?,
            animation,
            appearance,
            elapsed: Duration::ZERO,
            pressed_target: None,
            reduced_motion,
        })
    }
}

impl SplashDemo {
    /// Classify the splash's private controls and passive regions.
    fn classify(&self, pointer: Pointer) -> DemoTarget {
        if pointer.y >= 0.0 && pointer.y < 32.0 {
            if (512.0..558.0).contains(&pointer.x) {
                return DemoTarget::Close;
            }
            if (466.0..512.0).contains(&pointer.x) {
                return DemoTarget::Toggle;
            }
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
    /// Feed demonstration completion from the driver, never from the splash renderer.
    fn render(
        &mut self,
        width: u32,
        height: u32,
        scale: f64,
        elapsed: Duration,
    ) -> Result<Frame, Box<dyn Error>> {
        self.elapsed = elapsed;
        self.animation.tick(elapsed);
        let cycle = elapsed.as_secs_f32() % 12.0;
        let completed = (cycle / 10.0).min(1.0);
        let activity = if self.reduced_motion || completed == 1.0 {
            None
        } else {
            Some((elapsed.as_secs_f32() % 1.8) / 1.8)
        };
        let pixels = self.renderer.render(
            width,
            height,
            scale,
            self.appearance,
            self.animation.frame(elapsed),
            ProgressFrame { completed, activity },
        )?;
        Ok(Frame { width, height, premultiplied_rgba: pixels.take() })
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
    fn pointer(&mut self, action: PointerAction, pointer: Pointer) {
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
                    self.animation.trigger_flip(self.elapsed);
                }
            }
            PointerAction::Release => {
                if self.pressed_target == Some(target) && target == DemoTarget::Toggle {
                    self.appearance.expanded = !self.appearance.expanded;
                }
                self.appearance.pressed = None;
                self.pressed_target = None;
            }
        }
    }

    /// Route the adapter's keyboard disclosure intent to the same view state.
    fn key(&mut self, action: KeyAction) {
        match action {
            KeyAction::Tab => self.appearance.expanded = !self.appearance.expanded,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cancel an interrupted disclosure click and accept a matching press/release.
    #[test]
    fn disclosure_requires_matching_control_and_retains_blank_output() -> Result<(), Box<dyn Error>>
    {
        let mut demo = SplashDemo::new("en", Appearance::demo(), false)?;
        let toggle = Pointer { x: 480.0, y: 16.0 };
        demo.pointer(PointerAction::Press, toggle);
        demo.pointer(PointerAction::Release, Pointer { x: 10.0, y: 16.0 });
        assert!(demo.appearance.expanded);
        demo.pointer(PointerAction::Press, toggle);
        demo.pointer(PointerAction::Release, toggle);
        assert!(!demo.appearance.expanded);
        demo.key(KeyAction::Tab);
        assert!(demo.appearance.expanded);
        assert_eq!(demo.hit_test(Pointer { x: 100.0, y: 400.0 }), HitTarget::Client);
        Ok(())
    }

    /// Give the console overlay input priority over the covered mascot rectangle.
    #[test]
    fn covered_character_does_not_receive_console_clicks() -> Result<(), Box<dyn Error>> {
        let mut demo = SplashDemo::new("en", Appearance::demo(), false)?;
        let overlap = Pointer { x: 200.0, y: 380.0 };
        assert_eq!(demo.classify(overlap), DemoTarget::Console);
        let before = demo.animation.frame(Duration::from_millis(100));
        demo.pointer(PointerAction::Press, overlap);
        assert_eq!(demo.animation.frame(Duration::from_millis(100)), before);
        demo.pointer(PointerAction::Press, Pointer { x: 200.0, y: 200.0 });
        assert_ne!(demo.animation.frame(Duration::from_millis(100)), before);
        Ok(())
    }
}
