// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Compose production desktop roles, the native splash and isolated qualification fixtures.

extern crate self as sugarsubstitute;

mod capture;
#[cfg(all(target_os = "windows", feature = "windows-native-host"))]
mod capture_fixture;
#[cfg(all(target_os = "windows", feature = "windows-native-host"))]
mod claim_fixture;
#[cfg(all(windows, feature = "windows-native-host"))]
mod close_diagnostic;
mod demo;
#[cfg(all(target_os = "windows", feature = "windows-native-host"))]
mod descendant_fixture;
mod diagnostics;
#[cfg(all(target_os = "windows", feature = "windows-native-host"))]
mod forwarding_root;
#[cfg(all(target_os = "windows", feature = "windows-native-host"))]
mod held_child;
#[cfg(all(target_os = "windows", feature = "windows-native-host"))]
mod held_fixture;
pub mod instance_conflict;
mod invocation;
pub mod job_sequence;
#[cfg(all(target_os = "windows", feature = "windows-native-host"))]
mod output_fixture;
#[cfg(all(target_os = "windows", feature = "windows-native-host"))]
mod pipe_fixture;
mod preparation;
mod preparation_task;
#[cfg(all(target_os = "windows", feature = "windows-native-host"))]
mod process_fixture;
#[cfg(all(target_os = "windows", feature = "windows-native-host"))]
mod production;
#[cfg(all(target_os = "windows", feature = "windows-native-host"))]
mod production_output;
mod render_transfer;
pub mod startup;
mod timing_receipt;
mod transcript_snapshot;
pub use transcript_snapshot::{OutputSnapshot, SourceCustody};

use std::{error::Error, process::ExitCode, time::Instant};
use sugarsubstitute_contracts::localization::messages;
use sugarsubstitute_desktop::{DesktopOptions, run as run_desktop};
use sugarsubstitute_splash::Theme;

/// Dispatch actual production roles or isolated native/off-screen qualification.
fn execute(health: diagnostics::DiagnosticHealth) -> Result<(), Box<dyn Error>> {
    health.check()?;
    let started = Instant::now();
    #[cfg(all(target_os = "windows", feature = "windows-native-host"))]
    let invocation = {
        let Some(arguments) = production::execute(std::env::args_os().skip(1), health.clone())?
        else {
            return Ok(());
        };
        invocation::Invocation::parse_arguments(arguments)
    };
    #[cfg(not(all(target_os = "windows", feature = "windows-native-host")))]
    let invocation = invocation::Invocation::parse();
    let options = match invocation {
        Ok(invocation::Invocation::Demo(options)) => options,
        #[cfg(all(target_os = "windows", feature = "windows-native-host"))]
        Ok(invocation::Invocation::NativeCaptureFixture(options)) => {
            return capture_fixture::run(options)?.finish_with(|path, bytes| {
                std::fs::write(path, bytes)?;
                Ok(())
            });
        }
        #[cfg(all(target_os = "windows", feature = "windows-native-host"))]
        Ok(invocation::Invocation::NativeExitFixture(code)) => std::process::exit(code),
        #[cfg(all(target_os = "windows", feature = "windows-native-host"))]
        Ok(invocation::Invocation::NativeOutputChild(options)) => {
            std::process::exit(output_fixture::run_child(options, health.clone())?);
        }
        #[cfg(all(target_os = "windows", feature = "windows-native-host"))]
        Ok(invocation::Invocation::NativeOutputFixture(path)) => {
            let receipt = output_fixture::run()?;
            std::fs::write(path, serde_json::to_vec_pretty(&receipt)?)?;
            return Ok(());
        }
        #[cfg(all(target_os = "windows", feature = "windows-native-host"))]
        Ok(invocation::Invocation::NativeForwardingRoot(options)) => {
            std::process::exit(forwarding_root::run(options)?);
        }
        #[cfg(all(target_os = "windows", feature = "windows-native-host"))]
        Ok(invocation::Invocation::NativeDescendantFixture(path)) => {
            let receipt = descendant_fixture::run()?;
            std::fs::write(path, serde_json::to_vec_pretty(&receipt)?)?;
            return Ok(());
        }
        #[cfg(all(target_os = "windows", feature = "windows-native-host"))]
        Ok(invocation::Invocation::NativeClaimFixture(options)) => {
            let path = options.receipt.clone();
            let receipt = claim_fixture::run(options)?;
            std::fs::write(path, serde_json::to_vec_pretty(&receipt)?)?;
            return Ok(());
        }
        #[cfg(all(target_os = "windows", feature = "windows-native-host"))]
        Ok(invocation::Invocation::NativeClaimChild(options)) => {
            let result = claim_fixture::run_child(options)?;
            std::fs::write(result.path, serde_json::to_vec_pretty(&result.receipt)?)?;
            std::process::exit(result.exit_code);
        }
        #[cfg(all(target_os = "windows", feature = "windows-native-host"))]
        Ok(invocation::Invocation::NativeHeldChild(options)) => {
            let code = held_child::run(options)?;
            std::process::exit(code);
        }
        #[cfg(all(target_os = "windows", feature = "windows-native-host"))]
        Ok(invocation::Invocation::NativeRegistryFixture(path)) => {
            let receipt = process_fixture::run()?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, serde_json::to_vec_pretty(&receipt)?)?;
            return Ok(());
        }
        #[cfg(all(target_os = "windows", feature = "windows-native-host"))]
        Ok(invocation::Invocation::NativeHeldRegistryFixture(path)) => {
            let receipt = held_fixture::run()?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, serde_json::to_vec_pretty(&receipt)?)?;
            return Ok(());
        }
        #[cfg(all(target_os = "windows", feature = "windows-native-host"))]
        Ok(invocation::Invocation::NativePipeChild(options)) => {
            let code = pipe_fixture::run_child(options)?;
            std::process::exit(code);
        }
        #[cfg(all(target_os = "windows", feature = "windows-native-host"))]
        Ok(invocation::Invocation::NativePipeFixture(path)) => {
            let receipt = pipe_fixture::run()?;
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, serde_json::to_vec_pretty(&receipt)?)?;
            return Ok(());
        }
        Err(error) => {
            return Err(error);
        }
    };
    if let (Some(scenarios), Some(output)) = (&options.scenarios, &options.output) {
        capture::export(scenarios, output)?;
        return Ok(());
    }
    let content =
        demo::SplashDemo::new(&options.locale, options.appearance, options.reduced_motion)?;
    let before_native = started.elapsed();
    let receipt = run_desktop(
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

/// Run the selected native application role and preserve errors in structured diagnostics.
/// Must be called on the platform UI thread exactly once per process.
pub fn run() -> ExitCode {
    match run_application() {
        Ok(()) => ExitCode::SUCCESS,
        Err(_retained) => ExitCode::FAILURE,
    }
}

/// Observe original diagnostic health after the actual application lifetime and its final event.
fn run_application() -> Result<(), Box<dyn Error>> {
    let diagnostics = diagnostics::Diagnostics::initialize()?;
    let result = execute(diagnostics.health());
    if let Err(error) = &result {
        tracing::error!(operation="application_startup",cause=%error,"SugarSubstitute could not complete its application lifetime");
    }
    diagnostics.finish(result)
}

/// Product identity shared by every platform and packaging adapter.
pub const APPLICATION_NAME: &str = "SugarSubstitute";

#[cfg(test)]
mod identity_tests {
    /// Preserve the application identity expected by existing installations.
    #[test]
    fn product_name_preserves_existing_identity() {
        assert_eq!(super::APPLICATION_NAME, "SugarSubstitute");
    }
}

#[cfg(all(test, windows, feature = "k1-native-qualification"))]
mod desktop_error_fixture;
