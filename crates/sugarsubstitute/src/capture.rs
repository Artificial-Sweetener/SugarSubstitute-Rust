// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Export deterministic production-renderer frames for external visual qualification.

use serde::Deserialize;
use std::{error::Error, path::Path};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_contracts::localization::LOCALES;
use sugarsubstitute_splash::{AnimationFrame, Appearance, Control, SplashRenderer, Theme};
use sugarsubstitute_supervision::{ArtifactPublication, read_artifact, write_artifact};
use sugarsubstitute_ui::progress::ProgressFrame;

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

/// Admit the finite scenario inventory during decoding, before allocating another element.
struct Scenarios(Vec<Scenario>);
impl<'de> Deserialize<'de> for Scenarios {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Scenarios;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("1 through 1024 parity scenarios")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Scenarios, A::Error> {
                let mut scenarios = Vec::new();
                for _ in 0..1024 {
                    let Some(scenario) = sequence.next_element()? else {
                        if scenarios.is_empty() {
                            return Err(serde::de::Error::custom(
                                "Parity input requires 1 through 1024 scenarios",
                            ));
                        }
                        return Ok(Scenarios(scenarios));
                    };
                    scenarios.push(scenario);
                }
                if sequence.next_element::<serde::de::IgnoredAny>()?.is_some() {
                    return Err(serde::de::Error::custom(
                        "Parity input requires 1 through 1024 scenarios",
                    ));
                }
                Ok(Scenarios(scenarios))
            }
        }
        decoder.deserialize_seq(Visitor)
    }
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
fn control(value: Option<&str>) -> std::io::Result<Option<Control>> {
    match value {
        None => Ok(None),
        Some("console") => Ok(Some(Control::ConsoleToggle)),
        Some("close") => Ok(Some(Control::Close)),
        Some(_) => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Parity scenario control is invalid",
        )),
    }
}

/// Render explicit fixture scenarios without a window or native compositor.
pub(crate) fn export(scenarios: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
    let scenarios = scenarios.to_owned();
    let output = output.to_owned();
    crate::artifact_job::run(crate::artifact_job::Kind::Raster, move |context| {
        export_admitted(&scenarios, &output, &context)
            .map_err(crate::artifact_job::WorkError::boxed)
    })
}

/// Hold scenario, renderer and encode allocations in the same original artifact reservation.
fn export_admitted(
    scenarios: &Path,
    output: &Path,
    context: &JobContext,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let source = read_artifact(scenarios, 16 * 1024 * 1024, context)?;
    let _parsed = context.memory().claim(32 * 1024 * 1024)?;
    let Scenarios(definitions) = serde_json::from_slice(source.as_slice())?;
    drop(source);
    let mut locale = canonical_locale();
    let mut renderer = SplashRenderer::new_admitted(&locale, context.memory().clone())?;
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
            drop(renderer);
            renderer = SplashRenderer::new_admitted(&locale, context.memory().clone())?;
        }
        let appearance = Appearance {
            status: sugarsubstitute_splash::StartupStatus::Loading,
            operation: Default::default(),
            theme,
            // Parity scenarios compare against the reference's product accent.
            accent: sugarsubstitute_ui::accent::AccentPalette::PRODUCT,
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
        let _frame_workspace =
            context.memory().claim(3 * (size as u64) * (size as u64) * 4 + 8 * 1024 * 1024)?;
        let frame = renderer.render(
            size as u32,
            size as u32,
            scenario.scale,
            appearance,
            character,
            feedback,
        )?;
        write_artifact(
            &output.join(format!("{}.png", scenario.name)),
            true,
            ArtifactPublication::ReplaceSelected,
            64 * 1024 * 1024,
            context,
            |target| frame.write_png(target),
        )?;
        if scenario.raw {
            write_artifact(
                &output.join(format!("{}.raw", scenario.name)),
                true,
                ArtifactPublication::ReplaceSelected,
                frame.data().len(),
                context,
                |target| target.write_all(frame.data()),
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reject excess inventory while decoding rather than materializing an unbounded vector.
    #[test]
    fn scenario_inventory_enforces_the_existing_finite_limit_during_decoding()
    -> Result<(), Box<dyn Error>> {
        let scenario = r#"{"name":"pose","theme":"dark","expanded":false,"scale":1.0,"flip":0.0,"fill":0.4,"sweep":0.5}"#;
        let maximum = format!("[{}]", vec![scenario; 1024].join(","));
        assert_eq!(serde_json::from_str::<Scenarios>(&maximum)?.0.len(), 1024);
        let excessive = format!("[{}]", vec![scenario; 1025].join(","));
        assert!(serde_json::from_str::<Scenarios>(&excessive).is_err());
        assert!(serde_json::from_str::<Scenarios>("[]").is_err());
        Ok(())
    }

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
