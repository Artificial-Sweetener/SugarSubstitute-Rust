// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Construct ComfyUI managed argv before semantic backend identity is resolved.
//! Schema-one configuration arguments remain extra ComfyUI options. This pure
//! conversion performs no path resolution, environment modification or launch.
//! ComfyUI's `comfy/cli_args.py` owns option value types and operational semantics.
use std::{error::Error, fmt};
use sugarsubstitute_contracts::configuration::{ConfigurationError, LaunchConfiguration};
mod options;

/// Reject an effective command without exposing supplied paths or option values.
#[derive(Debug)]
pub enum ManagedLaunchError {
    /// An extra option can override a managed server or installed frontend invariant.
    ProtectedOption,
    /// Only supported complete option names are admitted, without abbreviations.
    UnknownOption,
    /// An option's value count, equals syntax or a standalone positional token is invalid.
    ArgumentShape,
    /// The complete effective command exceeds the existing domain budgets.
    Configuration(ConfigurationError),
}
impl fmt::Display for ManagedLaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ProtectedOption => "Extra ComfyUI option conflicts with managed startup",
            Self::UnknownOption => "Extra ComfyUI option requires a supported complete name",
            Self::ArgumentShape => "Extra ComfyUI arguments have invalid option or value shape",
            Self::Configuration(_) => "Effective ComfyUI command exceeds launch constraints",
        })
    }
}
impl Error for ManagedLaunchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Configuration(cause) => Some(cause),
            _ => None,
        }
    }
}

/// Prepare `-u main.py --listen 127.0.0.1 --port PORT --disable-auto-launch` plus extras.
///
/// The returned configuration supplies effective arguments for backend semantic
/// identity. Its lexical root/interpreter still require the resource owner's
/// canonical retained-file validation. The environment stays with that owner.
/// Reject managed listen/port, TLS, browser launch, frontend replacement/download,
/// custom-node suppression and exit-only options, including their abbreviations.
/// Supported option values retain exact Unicode, empty values and ordering.
/// Final argument count, individual and aggregate byte limits include this prefix.
pub fn effective_managed_launch(
    requested: &LaunchConfiguration,
) -> Result<LaunchConfiguration, ManagedLaunchError> {
    options::validate(requested.arguments())?;
    let script = requested.root().join("main.py");
    let script = script.to_str().ok_or(ManagedLaunchError::ArgumentShape)?;
    let mut arguments = vec![
        "-u".into(),
        script.into(),
        "--listen".into(),
        "127.0.0.1".into(),
        "--port".into(),
        requested.port().to_string(),
        "--disable-auto-launch".into(),
    ];
    arguments.extend(requested.arguments().iter().cloned());
    LaunchConfiguration::new(
        requested.root().into(),
        requested.interpreter().into(),
        arguments,
        requested.port().get(),
    )
    .map_err(ManagedLaunchError::Configuration)
}

#[cfg(test)]
mod tests;
