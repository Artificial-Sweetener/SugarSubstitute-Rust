// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Compose SugarSubstitute's native splash proof independently of future startup services.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod capture;
mod demo;

use std::{
    error::Error,
    path::PathBuf,
    process::ExitCode,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::localization::messages;
use sugarsubstitute_desktop::{DesktopOptions, run};
use sugarsubstitute_splash::{Appearance, Theme};

/// Hold explicit demonstration and qualification invocation inputs.
struct Options {
    appearance: Appearance,
    locale: String,
    reduced_motion: bool,
    duration: Option<Duration>,
    receipt: Option<PathBuf>,
    frame_audit: Option<PathBuf>,
    scenarios: Option<PathBuf>,
    output: Option<PathBuf>,
}

impl Options {
    /// Parse only implemented controls, rejecting incomplete or unknown options.
    fn parse() -> Result<Self, Box<dyn Error>> {
        let mut options = Self {
            appearance: Appearance::demo(),
            locale: "en".into(),
            reduced_motion: false,
            duration: None,
            receipt: None,
            frame_audit: None,
            scenarios: None,
            output: None,
        };
        let mut arguments = std::env::args_os().skip(1);
        while let Some(argument) = arguments.next() {
            match argument.to_str().ok_or("Option name is not valid Unicode")? {
                "--collapsed" => options.appearance.expanded = false,
                "--light" => options.appearance.theme = Theme::Light,
                "--reduced-motion" => options.reduced_motion = true,
                "--locale" => {
                    options.locale = arguments
                        .next()
                        .ok_or("--locale requires a value")?
                        .into_string()
                        .map_err(|_| "Locale is not valid Unicode")?
                }
                "--duration-ms" => {
                    let value = arguments.next().ok_or("--duration-ms requires a value")?;
                    let milliseconds =
                        value.to_str().ok_or("Duration is not valid Unicode")?.parse()?;
                    options.duration = Some(Duration::from_millis(milliseconds));
                }
                "--receipt" => {
                    options.receipt =
                        Some(arguments.next().ok_or("--receipt requires a path")?.into())
                }
                "--frame-audit" => {
                    options.frame_audit =
                        Some(arguments.next().ok_or("--frame-audit requires a path")?.into())
                }
                "--scenarios" => {
                    options.scenarios =
                        Some(arguments.next().ok_or("--scenarios requires a path")?.into())
                }
                "--output" => {
                    options.output =
                        Some(arguments.next().ok_or("--output requires a path")?.into())
                }
                _ => return Err("Unknown SugarSubstitute proof option".into()),
            }
        }
        if options.scenarios.is_some() != options.output.is_some() {
            return Err("--scenarios and --output must be supplied together".into());
        }
        if options.frame_audit.is_some()
            && (options.receipt.is_none() || options.scenarios.is_some())
        {
            return Err("--frame-audit requires --receipt in native qualification mode".into());
        }
        Ok(options)
    }
}

/// Run the interactive native proof or deterministic off-screen qualification.
fn execute() -> Result<(), Box<dyn Error>> {
    let started = Instant::now();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_writer(std::io::stderr)
        .try_init()
        .map_err(|error| error as Box<dyn Error>)?;
    let options = Options::parse()?;
    if let (Some(scenarios), Some(output)) = (&options.scenarios, &options.output) {
        capture::export(scenarios, output)?;
        return Ok(());
    }
    let content =
        demo::SplashDemo::new(&options.locale, options.appearance, options.reduced_motion)?;
    let before_native = started.elapsed();
    let receipt = run(
        DesktopOptions {
            title: messages(&options.locale).loading.into(),
            logical_width: 558,
            logical_height: 558,
            dark: options.appearance.theme == Theme::Dark,
            duration: options.duration,
            audit_frames: options.frame_audit.is_some(),
        },
        content,
    )?;
    tracing::info!(operation="splash_demo", first_frame_ms=(before_native+receipt.first_frame).as_secs_f64()*1000.0, frames=receipt.frames_presented, material=?receipt.material, "Native splash proof finished");
    let (frame_audit, native_move_audit) = match (options.frame_audit, receipt.frame_audit) {
        (Some(path), Some(audit)) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, &audit.first_frame.premultiplied_rgba)?;
            let frame_audit = serde_json::json!({
                "width": audit.first_frame.width,
                "height": audit.first_frame.height,
                "presented_frames": audit.presented_frames,
                "content_changes": audit.content_changes,
                "byte_count": audit.first_frame.premultiplied_rgba.len(),
                "source_path": path,
            });
            let presentations: Vec<_> = audit
                .move_presentations
                .iter()
                .map(|sample| {
                    serde_json::json!({
                        "sequence": sample.sequence,
                        "elapsed_ms": sample.elapsed.as_secs_f64() * 1000.0,
                        "source_revision": sample.source_revision,
                    })
                })
                .collect();
            let native_move_audit = serde_json::json!({
                "schema_version": 1,
                "move_frames_observed": audit.move_frames_observed,
                "move_samples_omitted": audit.move_samples_omitted,
                "presentations": presentations,
            });
            (frame_audit, native_move_audit)
        }
        (None, None) => (serde_json::Value::Null, serde_json::Value::Null),
        _ => return Err("Native frame audit did not match the requested qualification".into()),
    };
    if let Some(path) = options.receipt {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::json!({
            "entry_to_first_submission_ms":(before_native+receipt.first_frame).as_secs_f64()*1000.0,
            "native_entry_to_first_submission_ms":receipt.first_frame.as_secs_f64()*1000.0,
            "frames_presented":receipt.frames_presented,
            "material":format!("{:?}",receipt.material),
            "mean_frame_work_ms":receipt.total_frame_work.as_secs_f64()*1000.0 / receipt.frames_presented.max(1) as f64,
            "mean_frame_interval_ms":receipt.total_frame_intervals.as_secs_f64()*1000.0 / receipt.frames_presented.saturating_sub(1).max(1) as f64,
            "longest_frame_interval_ms":receipt.longest_frame_interval.as_secs_f64()*1000.0,
            "frame_audit": frame_audit,
            "native_move_audit": native_move_audit,
            "elapsed_ms":started.elapsed().as_secs_f64()*1000.0
        });
        std::fs::write(path, serde_json::to_vec_pretty(&data)?)?;
    }
    Ok(())
}

/// Preserve errors in structured diagnostics and return a failing process status.
fn main() -> ExitCode {
    match execute() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(operation="splash_proof",cause=%error,"SugarSubstitute could not show the native proof");
            ExitCode::FAILURE
        }
    }
}
