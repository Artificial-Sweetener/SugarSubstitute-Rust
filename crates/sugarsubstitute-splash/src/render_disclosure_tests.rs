// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify actual disclosure pixels, status suppression and animation geometry offscreen.
use crate::{AnimationFrame, Appearance, SplashRenderer, StartupStatus, Theme};
use std::error::Error;
use sugarsubstitute_ui::progress::ProgressFrame;

#[test]
fn disclosed_and_transitioning_frames_hide_status_without_hiding_progress()
-> Result<(), Box<dyn Error>> {
    let mut renderer = SplashRenderer::new("en")?;
    for theme in [Theme::Dark, Theme::Light] {
        for scale in [1.0, 1.5, 2.0] {
            for reveal in [0.0, 0.05, 0.5, 1.0] {
                for expanded in [false, true] {
                    let appearance = Appearance { theme, expanded, reveal, ..Appearance::demo() };
                    let size = (558.0 * scale) as u32;
                    let progress = ProgressFrame { completed: 0.4, activity: None };
                    let a = renderer.render(
                        size,
                        size,
                        scale,
                        appearance,
                        AnimationFrame::bootstrap(),
                        progress,
                    )?;
                    let b = renderer.render(
                        size,
                        size,
                        scale,
                        Appearance { status: StartupStatus::LoadingLong, ..appearance },
                        AnimationFrame::bootstrap(),
                        progress,
                    )?;
                    if appearance.status_visible() {
                        assert_ne!(a.data(), b.data());
                    } else {
                        assert_eq!(a.data(), b.data());
                    }
                    let x = (100.0 * scale) as u32;
                    let y = ((appearance.bar_y() + 2.0) * scale) as u32;
                    let bar = a.pixel(x, y).ok_or("progress pixel absent")?;
                    assert!(bar.red() > bar.green(), "single startup bar lost at reveal {reveal}");
                }
            }
        }
    }
    Ok(())
}

#[test]
fn invalid_disclosure_samples_are_rejected_before_raster_work() -> Result<(), Box<dyn Error>> {
    let mut renderer = SplashRenderer::new("en")?;
    for reveal in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        assert!(
            renderer
                .render(
                    558,
                    558,
                    1.0,
                    Appearance { reveal, ..Appearance::demo() },
                    AnimationFrame::bootstrap(),
                    ProgressFrame { completed: 0.4, activity: None }
                )
                .is_err()
        );
    }
    Ok(())
}
