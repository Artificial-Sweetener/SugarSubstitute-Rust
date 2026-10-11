// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Export deterministic production-renderer frames for external visual qualification.

use serde::Deserialize;
use std::{error::Error, fs, path::Path};
use sugarsubstitute_splash::{AnimationFrame, Appearance, ProgressFrame, SplashRenderer, Theme};

/// Read a parity scenario emitted by the production PySide reference harness.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Scenario {
    name: String,
    theme: String,
    expanded: bool,
    scale: f64,
    flip: f64,
    fill: f32,
    sweep: f32,
    #[serde(default)]
    pose: usize,
    #[serde(default = "forward_direction")]
    direction: i8,
}

/// Preserve the original forward direction for reference harnesses predating direction cases.
fn forward_direction() -> i8 {
    1
}

/// Render explicit fixture scenarios without a window or native compositor.
pub(crate) fn export(scenarios: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
    let definitions: Vec<Scenario> = serde_json::from_slice(&fs::read(scenarios)?)?;
    if definitions.is_empty() || definitions.len() > 1024 {
        return Err("Parity input requires 1 through 1024 scenarios".into());
    }
    fs::create_dir_all(output)?;
    let mut renderer = SplashRenderer::new("en")?;
    for scenario in definitions {
        if scenario.name.is_empty()
            || !scenario
                .name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
            || scenario.name == "."
            || scenario.name == ".."
        {
            return Err("Parity scenario filename is invalid".into());
        }
        let theme = match scenario.theme.as_str() {
            "dark" => Theme::Dark,
            "light" => Theme::Light,
            _ => return Err("Parity scenario theme is invalid".into()),
        };
        if !scenario.flip.is_finite() || !(0.0..=1.0).contains(&scenario.flip) {
            return Err("Parity scenario flip is invalid".into());
        }
        let appearance =
            Appearance { theme, expanded: scenario.expanded, hovered: None, pressed: None };
        let character = AnimationFrame {
            pose: if scenario.flip >= 0.5 { 1 } else { scenario.pose },
            progress: scenario.flip,
            direction: scenario.direction,
        };
        let feedback = ProgressFrame {
            completed: scenario.fill,
            activity: if scenario.fill == 1.0 { None } else { Some(scenario.sweep) },
        };
        let size = (558.0 * scenario.scale).round();
        if !size.is_finite() || !(1.0..=4096.0).contains(&size) {
            return Err("Parity scenario size is invalid".into());
        }
        let frame = renderer.render(
            size as u32,
            size as u32,
            scenario.scale,
            appearance,
            character,
            feedback,
        )?;
        frame.save_png(output.join(format!("{}.png", scenario.name)))?;
    }
    Ok(())
}
