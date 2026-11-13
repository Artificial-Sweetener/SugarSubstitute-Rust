// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The splash says which version it is updating to, in every locale. Set
//! `SUGARSUBSTITUTE_REPORT_PROOF_DIR` to keep the rendered frames as PNG files.
use super::SplashRenderer;
use crate::{AnimationFrame, Appearance};
use sugarsubstitute_ui::progress::ProgressFrame;

/// "Updating to version X…" replaces the loading words in the same status clip, in every
/// locale, and leaves the character and caption untouched.
#[test]
fn updating_status_names_the_version_in_the_status_clip() -> Result<(), Box<dyn std::error::Error>>
{
    let feedback = ProgressFrame { completed: 0.0, activity: None };
    for locale in
        sugarsubstitute_contracts::localization::LOCALES.iter().map(|messages| messages.id)
    {
        let mut renderer = SplashRenderer::new(locale)?;
        let mut appearance = Appearance { expanded: false, reveal: 0.0, ..Appearance::demo() };
        let loading =
            renderer.render(558, 558, 1.0, appearance, AnimationFrame::bootstrap(), feedback)?;
        let version = crate::UpdateVersion::new("2.4.1").ok_or("version label")?;
        appearance.status = crate::StartupStatus::Updating(version);
        let updating =
            renderer.render(558, 558, 1.0, appearance, AnimationFrame::bootstrap(), feedback)?;
        let other = crate::UpdateVersion::new("3.0.0").ok_or("version label")?;
        appearance.status = crate::StartupStatus::Updating(other);
        let later =
            renderer.render(558, 558, 1.0, appearance, AnimationFrame::bootstrap(), feedback)?;
        let differs = |a: &crate::Raster, b: &crate::Raster| {
            (486..530).any(|y| (24..534).any(|x| a.pixel(x, y) != b.pixel(x, y)))
        };
        assert!(differs(&loading, &updating), "{locale}: words changed");
        assert!(differs(&updating, &later), "{locale}: the version is shown");
        for y in 0..558 {
            for x in 0..558 {
                if !(24..534).contains(&x) || !(486..530).contains(&y) {
                    assert_eq!(loading.pixel(x, y), updating.pixel(x, y));
                }
            }
        }
        if let Some(directory) = std::env::var_os("SUGARSUBSTITUTE_REPORT_PROOF_DIR") {
            std::fs::write(
                std::path::Path::new(&directory).join(format!("splash-updating-{locale}.png")),
                updating.encode_png()?,
            )?;
        }
    }
    Ok(())
}
