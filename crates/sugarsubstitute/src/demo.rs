// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Drive an explicit demonstration without claiming real application startup progress.

use std::{error::Error, time::Duration};
use sugarsubstitute_desktop::{
    AccessibleControl, ControlBounds, Frame, HitTarget, KeyAction, Pointer, PointerAction,
    WindowContent,
};
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
    bootstrap_presented: bool,
    enriched: bool,
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
            renderer: SplashRenderer::new(locale)?,
            animation,
            appearance,
            elapsed: Duration::ZERO,
            bootstrap_presented: false,
            enriched: false,
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
    /// Update the painted disclosure and its native accessible action together.
    fn toggle_output(&mut self) {
        self.appearance.expanded = !self.appearance.expanded;
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
        let cycle = if self.reduced_motion { 0.0 } else { elapsed.as_secs_f32() % 12.0 };
        let completed = (cycle / 10.0).min(1.0);
        let activity = if self.reduced_motion || completed == 1.0 {
            None
        } else {
            Some((elapsed.as_secs_f32() % 1.8) / 1.8)
        };
        let feedback = ProgressFrame { completed, activity };
        if !self.bootstrap_presented {
            let pixels =
                self.renderer.render_bootstrap(width, height, scale, self.appearance, feedback)?;
            self.bootstrap_presented = true;
            return Ok(Frame { width, height, premultiplied_rgba: pixels.take() });
        }
        let pixels = self.renderer.render(
            width,
            height,
            scale,
            self.appearance,
            self.animation.frame(elapsed),
            feedback,
        )?;
        self.enriched = true;
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
        animation_changed
            || before
                != (self.appearance.hovered, self.appearance.pressed, self.appearance.expanded)
    }

    /// Route the adapter's keyboard disclosure intent to the same view state.
    fn key(&mut self, action: KeyAction) -> bool {
        match action {
            KeyAction::Tab => self.toggle_output(),
        }
        true
    }

    /// Freeze both scripted progress and mascot motion when reduced motion is selected.
    fn animating(&self) -> bool {
        !self.reduced_motion || !self.enriched
    }

    /// Expose only the two existing caption controls through stable native identities.
    fn accessible_controls(&self) -> &[AccessibleControl] {
        &self.controls
    }

    /// Apply native assistive-technology disclosure through the pointer/keyboard state owner.
    fn activate(&mut self, id: u16) -> bool {
        if id == 1 {
            self.toggle_output();
            true
        } else {
            false
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

    /// Keep the caption draggable where its transparent paint overlaps the mascot.
    #[test]
    fn caption_input_precedes_the_character_beneath_it() -> Result<(), Box<dyn Error>> {
        for expanded in [false, true] {
            let mut appearance = Appearance::demo();
            appearance.expanded = expanded;
            let mut demo = SplashDemo::new("en", appearance, false)?;
            let caption = Pointer { x: 250.0, y: 16.0 };
            let before = demo.animation.frame(Duration::from_millis(100));
            assert_eq!(demo.hit_test(caption), HitTarget::Caption);
            assert!(!demo.pointer(PointerAction::Press, caption));
            assert_eq!(demo.animation.frame(Duration::from_millis(100)), before);
            assert_eq!(demo.classify(Pointer { x: 480.0, y: 16.0 }), DemoTarget::Toggle);
            assert_eq!(demo.hit_test(Pointer { x: 535.0, y: 16.0 }), HitTarget::Close);
            assert_eq!(demo.hit_test(Pointer { x: 250.0, y: 200.0 }), HitTarget::Client);
        }
        Ok(())
    }

    /// Ignore pointer motion across unchanged painted regions and redundant leaves.
    #[test]
    fn pointer_dirty_state_tracks_only_visible_control_changes() -> Result<(), Box<dyn Error>> {
        let mut demo = SplashDemo::new("en", Appearance::demo(), false)?;
        assert!(!demo.pointer(PointerAction::Move, Pointer { x: 10.0, y: 50.0 }));
        assert!(!demo.pointer(PointerAction::Move, Pointer { x: 11.0, y: 50.0 }));
        assert!(demo.pointer(PointerAction::Move, Pointer { x: 480.0, y: 16.0 }));
        assert!(!demo.pointer(PointerAction::Move, Pointer { x: 490.0, y: 16.0 }));
        assert!(demo.pointer(PointerAction::Leave, Pointer::default()));
        assert!(!demo.pointer(PointerAction::Leave, Pointer::default()));
        Ok(())
    }

    /// Publish bootstrap, enrich once, then sleep with static reduced-motion producer state.
    #[test]
    fn reduced_motion_enriches_once_and_repaints_only_meaningful_events()
    -> Result<(), Box<dyn Error>> {
        let mut demo = SplashDemo::new("en", Appearance::demo(), true)?;
        assert!(demo.animating());
        let bootstrap = demo.render(558, 558, 1.0, Duration::ZERO)?;
        assert!(demo.animating());
        let static_frame = demo.render(558, 558, 1.0, Duration::from_secs(1))?;
        assert!(!demo.animating());
        let later_frame = demo.render(558, 558, 1.0, Duration::from_secs(8))?;
        assert_eq!(static_frame.premultiplied_rgba, later_frame.premultiplied_rgba);
        assert_eq!(bootstrap.premultiplied_rgba, static_frame.premultiplied_rgba);
        assert!(!demo.pointer(PointerAction::Press, Pointer { x: 200.0, y: 200.0 }));
        assert!(demo.key(KeyAction::Tab));
        assert!(!demo.animating());
        Ok(())
    }

    /// Publish light-theme close feedback from dirty input while animation remains asleep.
    #[test]
    fn static_light_close_press_and_cancel_repaint_without_animation() -> Result<(), Box<dyn Error>>
    {
        let mut appearance = Appearance::demo();
        appearance.theme = sugarsubstitute_splash::Theme::Light;
        let mut demo = SplashDemo::new("en", appearance, true)?;
        demo.render(558, 558, 1.0, Duration::ZERO)?;
        let neutral = demo.render(558, 558, 1.0, Duration::from_secs(1))?;
        assert!(!demo.animating());
        let close = Pointer { x: 535.0, y: 16.0 };
        assert!(demo.pointer(PointerAction::Move, close));
        assert!(demo.pointer(PointerAction::Press, close));
        assert!(!demo.animating());
        let pressed = demo.render(558, 558, 1.0, Duration::from_secs(2))?;
        let sample = (8 * 558 + 518) * 4;
        assert_eq!(&pressed.premultiplied_rgba[sample..sample + 4], &[241, 112, 122, 255]);
        assert_ne!(neutral.premultiplied_rgba, pressed.premultiplied_rgba);
        assert!(demo.pointer(PointerAction::Leave, close));
        assert!(!demo.pointer(PointerAction::Release, close));
        assert!(!demo.animating());
        let cancelled = demo.render(558, 558, 1.0, Duration::from_secs(3))?;
        assert_eq!(neutral.premultiplied_rgba, cancelled.premultiplied_rgba);
        Ok(())
    }

    /// Keep native localized control intent and all disclosure routes with the same owner.
    #[test]
    fn accessible_disclosure_reflects_pointer_keyboard_and_native_activation()
    -> Result<(), Box<dyn Error>> {
        for locale in sugarsubstitute_contracts::localization::LOCALES {
            let mut demo = SplashDemo::new(locale.id, Appearance::demo(), true)?;
            assert_eq!(demo.accessible_controls()[0].label, locale.hide_output);
            assert_eq!(demo.accessible_controls()[1].label, locale.cancel);
            assert!(demo.accessible_controls()[1].close);
            assert!(demo.activate(1));
            assert_eq!(demo.accessible_controls()[0].label, locale.show_output);
            assert!(!demo.activate(2));
            assert!(!demo.activate(0));
            assert!(demo.key(KeyAction::Tab));
            assert_eq!(demo.accessible_controls()[0].label, locale.hide_output);
            let toggle = Pointer { x: 480.0, y: 16.0 };
            demo.pointer(PointerAction::Press, toggle);
            demo.pointer(PointerAction::Release, toggle);
            assert_eq!(demo.accessible_controls()[0].label, locale.show_output);
        }
        Ok(())
    }
}
