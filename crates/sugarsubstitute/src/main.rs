// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Compose the native splash proof and isolated Windows process-lifetime fixtures.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod capture;
#[cfg(target_os = "windows")]
mod claim_fixture;
mod demo;
#[cfg(target_os = "windows")]
mod descendant_fixture;
#[cfg(target_os = "windows")]
mod forwarding_root;
#[cfg(target_os = "windows")]
mod held_child;
#[cfg(target_os = "windows")]
mod held_fixture;
mod invocation;
#[cfg(target_os = "windows")]
mod pipe_fixture;
mod preparation;
mod preparation_task;
#[cfg(target_os = "windows")]
mod process_fixture;
mod render_transfer;
mod timing_receipt;

use std::{error::Error, process::ExitCode, time::Instant};
use sugarsubstitute_contracts::localization::messages;
use sugarsubstitute_desktop::{DesktopOptions, run};
use sugarsubstitute_splash::Theme;

/// Dispatch isolated process fixtures or run native/off-screen splash qualification.
fn execute() -> Result<(), Box<dyn Error>> {
    let started = Instant::now();
    let options = match invocation::Invocation::parse() {
        Ok(invocation::Invocation::Demo(options)) => options,
        #[cfg(target_os = "windows")]
        Ok(invocation::Invocation::NativeExitFixture(code)) => std::process::exit(code),
        #[cfg(target_os = "windows")]
        Ok(invocation::Invocation::NativeForwardingRoot(options)) => {
            initialize_diagnostics()?;
            std::process::exit(forwarding_root::run(options)?);
        }
        #[cfg(target_os = "windows")]
        Ok(invocation::Invocation::NativeDescendantFixture(path)) => {
            initialize_diagnostics()?;
            let receipt = descendant_fixture::run()?;
            std::fs::write(path, serde_json::to_vec_pretty(&receipt)?)?;
            return Ok(());
        }
        #[cfg(target_os = "windows")]
        Ok(invocation::Invocation::NativeClaimFixture(options)) => {
            initialize_diagnostics()?;
            let path = options.receipt.clone();
            let receipt = claim_fixture::run(options)?;
            std::fs::write(path, serde_json::to_vec_pretty(&receipt)?)?;
            return Ok(());
        }
        #[cfg(target_os = "windows")]
        Ok(invocation::Invocation::NativeClaimChild(options)) => {
            initialize_diagnostics()?;
            let result = claim_fixture::run_child(options)?;
            std::fs::write(result.path, serde_json::to_vec_pretty(&result.receipt)?)?;
            std::process::exit(result.exit_code);
        }
        #[cfg(target_os = "windows")]
        Ok(invocation::Invocation::NativeHeldChild(options)) => {
            initialize_diagnostics()?;
            let code = held_child::run(options)?;
            std::process::exit(code);
        }
        #[cfg(target_os = "windows")]
        Ok(invocation::Invocation::NativeRegistryFixture(path)) => {
            initialize_diagnostics()?;
            let receipt = process_fixture::run()?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, serde_json::to_vec_pretty(&receipt)?)?;
            return Ok(());
        }
        #[cfg(target_os = "windows")]
        Ok(invocation::Invocation::NativeHeldRegistryFixture(path)) => {
            initialize_diagnostics()?;
            let receipt = held_fixture::run()?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, serde_json::to_vec_pretty(&receipt)?)?;
            return Ok(());
        }
        #[cfg(target_os = "windows")]
        Ok(invocation::Invocation::NativePipeChild(options)) => {
            initialize_diagnostics()?;
            let code = pipe_fixture::run_child(options)?;
            std::process::exit(code);
        }
        #[cfg(target_os = "windows")]
        Ok(invocation::Invocation::NativePipeFixture(path)) => {
            initialize_diagnostics()?;
            let receipt = pipe_fixture::run()?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, serde_json::to_vec_pretty(&receipt)?)?;
            return Ok(());
        }
        Err(error) => {
            initialize_diagnostics()?;
            return Err(error);
        }
    };
    initialize_diagnostics()?;
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
            audit_timings: options.timing_audit,
        },
        content,
    )?;
    tracing::info!(operation="splash_demo", first_frame_ms=(before_native+receipt.first_frame).as_secs_f64()*1000.0, frames=receipt.frames_presented, material=?receipt.material, "Native splash proof finished");
    let native_timings = match (options.timing_audit, receipt.timings) {
        (true, Some(timings)) => timing_receipt::encode(timings),
        (false, None) => serde_json::Value::Null,
        _ => return Err("Native timing audit did not match the requested qualification".into()),
    };
    let (frame_audit, native_move_audit) = match (options.frame_audit, receipt.frame_audit) {
        (Some(path), Some(audit)) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, audit.first_frame.pixels())?;
            let frame_audit = serde_json::json!({
                "width": audit.first_frame.width(),
                "height": audit.first_frame.height(),
                "presented_frames": audit.presented_frames,
                "content_changes": audit.content_changes,
                "byte_count": audit.first_frame.pixels().len(),
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
            "prepared_frames_presented":receipt.prepared_frames_presented,
            "retained_frame_exposes":receipt.retained_frame_exposes,
            "material":format!("{:?}",receipt.material),
            "mean_frame_work_ms":receipt.total_frame_work.as_secs_f64()*1000.0 / receipt.frames_presented.max(1) as f64,
            "mean_frame_interval_ms":receipt.total_frame_intervals.as_secs_f64()*1000.0 / receipt.frames_presented.saturating_sub(1).max(1) as f64,
            "longest_frame_interval_ms":receipt.longest_frame_interval.as_secs_f64()*1000.0,
            "frame_audit": frame_audit,
            "native_move_audit": native_move_audit,
            "native_timings": native_timings,
            "elapsed_ms":started.elapsed().as_secs_f64()*1000.0
        });
        std::fs::write(path, serde_json::to_vec_pretty(&data)?)?;
    }
    Ok(())
}

/// Initialize diagnostics for presentation and rejected arguments before returning errors.
fn initialize_diagnostics() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_writer(std::io::stderr)
        .try_init()
        .map_err(|error| error as Box<dyn Error>)
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
