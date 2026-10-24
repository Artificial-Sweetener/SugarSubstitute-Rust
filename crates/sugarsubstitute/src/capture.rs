// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Export deterministic production-renderer frames for external visual qualification.

use serde::Deserialize;
use std::{error::Error, fs, path::Path};
use sugarsubstitute_contracts::localization::LOCALES;
use sugarsubstitute_splash::{
    AnimationFrame, Appearance, Control, ProgressFrame, SplashRenderer, Theme,
};

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
    #[serde(default = "activity_enabled")]
    activity: bool,
    #[serde(default)]
    raw: bool,
    #[serde(default)]
    pose: usize,
    #[serde(default = "forward_direction")]
    direction: i8,
    #[serde(default = "canonical_locale")]
    locale: String,
    #[serde(default)]
    hovered: Option<String>,
    #[serde(default)]
    pressed: Option<String>,
}

/// Preserve the original forward direction for reference harnesses predating direction cases.
fn forward_direction() -> i8 {
    1
}

/// Match original scenarios to the canonical reference text.
fn canonical_locale() -> String {
    "en".into()
}

/// Keep existing animated parity scenarios active unless static capture is requested.
fn activity_enabled() -> bool {
    true
}

/// Reject mistyped reference controls rather than silently painting a different state.
fn control(value: Option<&str>) -> Result<Option<Control>, Box<dyn Error>> {
    match value {
        None => Ok(None),
        Some("console") => Ok(Some(Control::ConsoleToggle)),
        Some("close") => Ok(Some(Control::Close)),
        Some(_) => Err("Parity scenario control is invalid".into()),
    }
}

/// Render explicit fixture scenarios without a window or native compositor.
pub(crate) fn export(scenarios: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
    let definitions: Vec<Scenario> = serde_json::from_slice(&fs::read(scenarios)?)?;
    if definitions.is_empty() || definitions.len() > 1024 {
        return Err("Parity input requires 1 through 1024 scenarios".into());
    }
    fs::create_dir_all(output)?;
    let mut locale = canonical_locale();
    let mut renderer = SplashRenderer::new(&locale)?;
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
        if !LOCALES.iter().any(|messages| messages.id == scenario.locale) {
            return Err("Parity scenario locale is invalid".into());
        }
        if locale != scenario.locale {
            locale = scenario.locale;
            renderer = SplashRenderer::new(&locale)?;
        }
        let appearance = Appearance {
            status: sugarsubstitute_splash::StartupStatus::Loading,
            operation: Default::default(),
            theme,
            expanded: scenario.expanded,
            reveal: f64::from(scenario.expanded),
            hovered: control(scenario.hovered.as_deref())?,
            pressed: control(scenario.pressed.as_deref())?,
        };
        let character = AnimationFrame {
            pose: if scenario.flip >= 0.5 { 1 } else { scenario.pose },
            progress: scenario.flip,
            direction: scenario.direction,
        };
        let feedback = ProgressFrame {
            completed: scenario.fill,
            activity: if scenario.fill == 1.0 || !scenario.activity {
                None
            } else {
                Some(scenario.sweep)
            },
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
        fs::write(output.join(format!("{}.png", scenario.name)), frame.encode_png()?)?;
        if scenario.raw {
            fs::write(output.join(format!("{}.raw", scenario.name)), frame.data())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Preserve existing qualification inputs while admitting explicit localized controls.
    #[test]
    fn reference_scenarios_preserve_defaults_and_reject_unknown_fields()
    -> Result<(), Box<dyn Error>> {
        let source = r#"{"name":"pose","theme":"dark","expanded":false,"scale":1.0,"flip":0.0,"fill":0.4,"sweep":0.5}"#;
        let scenario: Scenario = serde_json::from_str(source)?;
        assert_eq!(scenario.locale, "en");
        assert_eq!(scenario.direction, 1);
        assert!(scenario.hovered.is_none());
        assert!(scenario.activity);
        assert!(!scenario.raw);
        let mut explicit: serde_json::Value = serde_json::from_str(source)?;
        explicit["locale"] = "ja".into();
        explicit["hovered"] = "console".into();
        explicit["pressed"] = "close".into();
        explicit["activity"] = false.into();
        explicit["raw"] = true.into();
        let scenario: Scenario = serde_json::from_value(explicit.clone())?;
        assert_eq!(scenario.locale, "ja");
        assert!(!scenario.activity);
        assert!(scenario.raw);
        assert_eq!(control(scenario.hovered.as_deref())?, Some(Control::ConsoleToggle));
        assert_eq!(control(scenario.pressed.as_deref())?, Some(Control::Close));
        assert!(control(Some("unknown")).is_err());
        explicit["unreviewed"] = true.into();
        assert!(serde_json::from_value::<Scenario>(explicit).is_err());
        Ok(())
    }
}
