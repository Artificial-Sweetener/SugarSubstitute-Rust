// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose a standalone canvas without workflow, browser or inference startup.
mod options;
use crate::native_canvas::session::Session;
use std::{error::Error, process::ExitCode};

/// Enter the standalone native viewer, retaining ordinary host diagnostic ownership.
/// Arguments select an explicit cache anchor and optional initial image; an empty viewer opens
/// the native system picker. Backend/window input is independent of ComfyUI connection state.
pub fn entry() -> ExitCode {
    let result = crate::diagnostics::Diagnostics::run(|health| {
        let result = (|| -> Result<(), Box<dyn Error>> {
            let mut options = options::Options::parse(std::env::args_os().skip(1))?;
            let proof = options.proof.take();
            let proof_window = options.proof_window;
            let title = sugarsubstitute_contracts::localization::messages(&options.locale)
                .viewer
                .title
                .to_owned();
            let window = sugarsubstitute_desktop::canvas::WindowOptions {
                title,
                extent: options.extent,
                visible: !proof_window,
            };
            let mut session = Session::new(
                options.into_config(),
                health,
                sugarsubstitute_execution::MonotonicClock::new(),
                crate::job_sequence::JobSequence::new(),
            )?;
            match proof {
                Some(paths) => session.run_proof(paths, proof_window.then_some(window)),
                None => sugarsubstitute_desktop::canvas::run(window, &mut session),
            }
        })();
        if let Err(cause) = &result {
            tracing::error!(operation="canvas_viewer", cause=%cause, "Standalone canvas could not finish its lifetime");
        }
        result
    });
    if result.is_ok() { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}
