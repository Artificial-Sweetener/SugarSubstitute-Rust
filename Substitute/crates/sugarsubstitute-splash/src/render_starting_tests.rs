// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The launcher's "Starting…" notice is the splash with its own words and no controls.
use super::SplashRenderer;
use crate::{AnimationFrame, Appearance, StartupStatus};
use sugarsubstitute_ui::progress::ProgressFrame;

/// Device pixels of the status clip, the only place the words differ.
fn status_clip(x: u32, y: u32) -> bool {
    (24..534).contains(&x) && (486..530).contains(&y)
}

/// Device pixels of the caption controls in the top-right corner.
fn caption(x: u32, y: u32) -> bool {
    x >= 440 && y < 60
}

/// The notice says it is starting, in every locale, and only the status clip changes.
#[test]
fn starting_status_replaces_the_loading_words_in_every_locale()
-> Result<(), Box<dyn std::error::Error>> {
    let feedback = ProgressFrame { completed: 0.0, activity: None };
    for locale in
        sugarsubstitute_contracts::localization::LOCALES.iter().map(|messages| messages.id)
    {
        let mut renderer = SplashRenderer::new(locale)?;
        let mut appearance = Appearance { expanded: false, reveal: 0.0, ..Appearance::demo() };
        let loading =
            renderer.render(558, 558, 1.0, appearance, AnimationFrame::bootstrap(), feedback)?;
        appearance.status = StartupStatus::Starting;
        let starting =
            renderer.render(558, 558, 1.0, appearance, AnimationFrame::bootstrap(), feedback)?;
        let mut words = false;
        for y in 0..558 {
            for x in 0..558 {
                if status_clip(x, y) {
                    words |= loading.pixel(x, y) != starting.pixel(x, y);
                } else {
                    assert_eq!(loading.pixel(x, y), starting.pixel(x, y), "{locale}: ({x}, {y})");
                }
            }
        }
        assert!(words, "{locale}: the starting words are shown");
    }
    Ok(())
}

/// Without controls, the caption corner shows no console or close button, and nothing else
/// in the frame changes. The disclosed frame draws no status words, so this needs no fonts.
#[test]
fn a_notice_without_controls_draws_no_caption_buttons() -> Result<(), Box<dyn std::error::Error>> {
    let feedback = ProgressFrame { completed: 0.0, activity: None };
    let mut renderer = SplashRenderer::new("en")?;
    let with = Appearance { status: StartupStatus::Starting, ..Appearance::demo() };
    let without = Appearance { controls: false, ..with };
    let drawn = renderer.render(558, 558, 1.0, with, AnimationFrame::bootstrap(), feedback)?;
    let bare = renderer.render(558, 558, 1.0, without, AnimationFrame::bootstrap(), feedback)?;
    let mut removed = false;
    for y in 0..558 {
        for x in 0..558 {
            if caption(x, y) {
                removed |= drawn.pixel(x, y) != bare.pixel(x, y);
            } else {
                assert_eq!(drawn.pixel(x, y), bare.pixel(x, y), "({x}, {y})");
            }
        }
    }
    assert!(removed, "the controls were drawn before and are gone now");
    Ok(())
}
