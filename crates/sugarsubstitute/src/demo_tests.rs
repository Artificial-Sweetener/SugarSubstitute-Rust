// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve splash interaction behavior through governed immutable frame delivery.

use super::*;

/// Cancel an interrupted disclosure click and accept a matching press/release.
#[test]
fn disclosure_requires_matching_control_and_retains_blank_output() -> Result<(), Box<dyn Error>> {
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
fn reduced_motion_enriches_once_and_repaints_only_meaningful_events() -> Result<(), Box<dyn Error>>
{
    let mut demo = SplashDemo::new("en", Appearance::demo(), true)?;
    assert!(demo.animating());
    let wake = test_support::start(&mut demo)?;
    let bootstrap = test_support::prepare(&mut demo, &wake, Duration::ZERO)?;
    assert!(demo.animating());
    let static_frame = test_support::prepare(&mut demo, &wake, Duration::from_secs(1))?;
    assert!(!demo.animating());
    let later_frame = test_support::prepare(&mut demo, &wake, Duration::from_secs(8))?;
    assert_eq!(static_frame.pixels(), later_frame.pixels());
    assert_eq!(bootstrap.pixels(), static_frame.pixels());
    assert!(!demo.pointer(PointerAction::Press, Pointer { x: 200.0, y: 200.0 }));
    assert!(demo.key(KeyAction::Tab));
    assert!(!demo.animating());
    test_support::finish(&mut demo, &wake)?;
    Ok(())
}

/// Publish light-theme close feedback from dirty input while animation remains asleep.
#[test]
fn static_light_close_press_and_cancel_repaint_without_animation() -> Result<(), Box<dyn Error>> {
    let mut appearance = Appearance::demo();
    appearance.theme = sugarsubstitute_splash::Theme::Light;
    let mut demo = SplashDemo::new("en", appearance, true)?;
    let wake = test_support::start(&mut demo)?;
    drop(test_support::prepare(&mut demo, &wake, Duration::ZERO)?);
    let neutral = test_support::prepare(&mut demo, &wake, Duration::from_secs(1))?;
    assert!(!demo.animating());
    let close = Pointer { x: 535.0, y: 16.0 };
    assert!(demo.pointer(PointerAction::Move, close));
    assert!(demo.pointer(PointerAction::Press, close));
    assert!(!demo.animating());
    let pressed = test_support::prepare(&mut demo, &wake, Duration::from_secs(2))?;
    let sample = (8 * 558 + 518) * 4;
    assert_eq!(&pressed.pixels()[sample..sample + 4], &[241, 112, 122, 255]);
    assert_ne!(neutral.pixels(), pressed.pixels());
    assert!(demo.pointer(PointerAction::Leave, close));
    assert!(!demo.pointer(PointerAction::Release, close));
    assert!(!demo.animating());
    let cancelled = test_support::prepare(&mut demo, &wake, Duration::from_secs(3))?;
    assert_eq!(neutral.pixels(), cancelled.pixels());
    test_support::finish(&mut demo, &wake)?;
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
        let wake = test_support::start(&mut demo)?;
        drop(test_support::prepare(&mut demo, &wake, Duration::ZERO)?);
        assert!(demo.activate(1));
        assert_eq!(demo.accessible_controls()[0].label, locale.hide_output);
        drop(test_support::prepare(&mut demo, &wake, Duration::from_millis(10))?);
        assert_eq!(demo.accessible_controls()[0].label, locale.show_output);
        assert!(!demo.activate(2));
        assert!(!demo.activate(0));
        assert!(demo.key(KeyAction::Tab));
        drop(test_support::prepare(&mut demo, &wake, Duration::from_millis(20))?);
        assert_eq!(demo.accessible_controls()[0].label, locale.hide_output);
        let toggle = Pointer { x: 480.0, y: 16.0 };
        demo.pointer(PointerAction::Press, toggle);
        demo.pointer(PointerAction::Release, toggle);
        drop(test_support::prepare(&mut demo, &wake, Duration::from_millis(30))?);
        assert_eq!(demo.accessible_controls()[0].label, locale.show_output);
        test_support::finish(&mut demo, &wake)?;
    }
    Ok(())
}

/// Preserve shutdown before native start and reject repeated execution lifetimes.
#[test]
fn execution_start_and_shutdown_preserve_one_owned_lifetime() -> Result<(), Box<dyn Error>> {
    let mut never_started = SplashDemo::new("en", Appearance::demo(), true)?;
    never_started.request_shutdown()?;
    assert!(never_started.poll_shutdown()?);
    never_started.finish_shutdown()?;
    assert!(never_started.start(test_support::Wake::new()).is_err());
    let mut demo = SplashDemo::new("en", Appearance::demo(), true)?;
    let wake = test_support::start(&mut demo)?;
    assert!(demo.start(wake.clone()).is_err());
    drop(test_support::prepare(&mut demo, &wake, Duration::ZERO)?);
    demo.request_shutdown()?;
    demo.request_shutdown()?;
    test_support::finish(&mut demo, &wake)?;
    assert!(demo.preparation.is_none() && demo.execution.is_none());
    Ok(())
}
