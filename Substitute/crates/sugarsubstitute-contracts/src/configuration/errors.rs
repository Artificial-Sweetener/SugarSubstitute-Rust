// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Describe invalid configuration without embedding supplied paths or arguments.

use std::{error::Error, fmt};

/// A startup value failed semantic validation before acquiring runtime resources.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigurationError {
    /// A root or interpreter is not an absolute, bounded, unambiguous path.
    InvalidLaunchPath,
    /// More than the accepted number of subprocess arguments were supplied.
    TooManyArguments,
    /// An individual UTF-8 argument exceeds its byte limit.
    ArgumentTooLong,
    /// The argument array exceeds its combined UTF-8 byte limit.
    ArgumentsTooLarge,
    /// An argument contains a NUL byte.
    ArgumentContainsNul,
    /// TCP port zero cannot identify an explicit backend endpoint.
    InvalidPort,
    /// The profile name is outside the isolated ASCII identifier grammar.
    InvalidProfile,
    /// The locale is absent from the authoritative supported registry.
    UnsupportedLocale,
    /// An attachment is not an exact supported loopback HTTP root origin.
    InvalidAttachOrigin,
    /// The URL's effective HTTP port differs from the configured port.
    AttachPortMismatch,
}

impl fmt::Display for ConfigurationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLaunchPath => {
                "Launch paths must be absolute bounded paths without NUL or parent traversal"
            }
            Self::TooManyArguments => "Launch accepts at most 64 arguments",
            Self::ArgumentTooLong => "Each launch argument must fit within 4096 UTF-8 bytes",
            Self::ArgumentsTooLarge => {
                "Launch arguments must fit within 65536 combined UTF-8 bytes"
            }
            Self::ArgumentContainsNul => "Launch arguments must not contain NUL",
            Self::InvalidPort => "Backend port must be nonzero",
            Self::InvalidProfile => {
                "Profile must contain 1 to 64 ASCII letters, digits, underscores or hyphens"
            }
            Self::UnsupportedLocale => "Startup locale must match a supported registry identity",
            Self::InvalidAttachOrigin => "Attach requires a loopback HTTP root origin",
            Self::AttachPortMismatch => "Attach origin and configured port must match",
        })
    }
}

impl Error for ConfigurationError {}
