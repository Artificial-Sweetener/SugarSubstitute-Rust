// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Validate explicit launch or attachment inputs independently of runtime adapters.
//!
//! Lexical launch paths express requested resources. They grant no filesystem or
//! process authority: the startup adapter must resolve and validate their trust
//! before acquiring resources. Construction performs no I/O.

mod errors;
mod identity;
mod origin;

pub use errors::ConfigurationError;
pub use identity::{LocaleId, ProfileId};
pub use origin::{LoopbackHost, LoopbackOrigin};

use std::{
    fmt,
    num::NonZeroU16,
    path::{Component, Path, PathBuf},
};

/// Maximum number of separately passed launch arguments.
pub const MAX_ARGUMENTS: usize = 64;
/// Maximum UTF-8 byte length of one launch argument.
pub const MAX_ARGUMENT_BYTES: usize = 4096;
/// Maximum combined UTF-8 byte length of launch arguments.
pub const MAX_TOTAL_ARGUMENT_BYTES: usize = 64 * 1024;

/// Select the appearance shared by graphics, text and native material.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    /// Dark Fluent palette.
    Dark,
    /// Light Fluent palette.
    Light,
}

/// New-application presentation choices validated against the locale registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Presentation {
    locale: LocaleId,
    theme: Theme,
    reduced_motion: bool,
}

impl Presentation {
    /// Validate the locale and retain explicit appearance and animation choices.
    ///
    /// Returns [`ConfigurationError::UnsupportedLocale`] for an unknown locale.
    pub fn new(
        locale: &str,
        theme: Theme,
        reduced_motion: bool,
    ) -> Result<Self, ConfigurationError> {
        Ok(Self { locale: LocaleId::new(locale)?, theme, reduced_motion })
    }

    /// Return the supported locale identity.
    pub fn locale(self) -> LocaleId {
        self.locale
    }
    /// Return the requested appearance.
    pub fn theme(self) -> Theme {
        self.theme
    }
    /// Return whether motion should be reduced.
    pub fn reduced_motion(self) -> bool {
        self.reduced_motion
    }
}

/// An explicit launch request awaiting filesystem trust and process-owner validation.
#[derive(Clone, PartialEq, Eq)]
pub struct LaunchConfiguration {
    root: PathBuf,
    interpreter: PathBuf,
    arguments: Vec<String>,
    port: NonZeroU16,
}

impl LaunchConfiguration {
    /// Validate absolute lexical paths, bounded argument bytes and a nonzero port.
    ///
    /// Returns a typed path, argument or port error. Arguments remain byte-exact;
    /// they must later be passed as an argument list rather than shell text.
    /// Paths are neither resolved nor trusted by this constructor.
    pub fn new(
        root: PathBuf,
        interpreter: PathBuf,
        arguments: Vec<String>,
        port: u16,
    ) -> Result<Self, ConfigurationError> {
        for path in [&root, &interpreter] {
            if !path.is_absolute()
                || path.to_str().is_none_or(|text| text.contains('\0'))
                || path
                    .components()
                    .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
            {
                return Err(ConfigurationError::InvalidLaunchPath);
            }
        }
        if arguments.len() > MAX_ARGUMENTS {
            return Err(ConfigurationError::TooManyArguments);
        }
        let mut total = 0;
        for argument in &arguments {
            if argument.contains('\0') {
                return Err(ConfigurationError::ArgumentContainsNul);
            }
            if argument.len() > MAX_ARGUMENT_BYTES {
                return Err(ConfigurationError::ArgumentTooLong);
            }
            total += argument.len();
        }
        if total > MAX_TOTAL_ARGUMENT_BYTES {
            return Err(ConfigurationError::ArgumentsTooLarge);
        }
        let port = NonZeroU16::new(port).ok_or(ConfigurationError::InvalidPort)?;
        Ok(Self { root, interpreter, arguments, port })
    }

    /// Borrow the requested absolute ComfyUI root; no trust is implied.
    pub fn root(&self) -> &Path {
        &self.root
    }
    /// Borrow the requested absolute interpreter path; no executable trust is implied.
    pub fn interpreter(&self) -> &Path {
        &self.interpreter
    }
    /// Borrow the exact bounded argument list.
    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }
    /// Return the requested nonzero backend port.
    pub fn port(&self) -> NonZeroU16 {
        self.port
    }
}

impl fmt::Debug for LaunchConfiguration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LaunchConfiguration")
            .field("argument_count", &self.arguments.len())
            .field("port", &self.port)
            .finish_non_exhaustive()
    }
}

/// Distinguish a requested managed launch from an externally owned attachment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectionConfiguration {
    /// Validate trust before asking the process owner to launch these resources.
    Launch(LaunchConfiguration),
    /// Connect to an external loopback service without claiming process ownership.
    Attach(LoopbackOrigin),
}

/// Validated startup intent containing no live resources or native privileges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartupConfiguration {
    connection: ConnectionConfiguration,
    profile: ProfileId,
    presentation: Presentation,
}

impl StartupConfiguration {
    /// Compose already validated connection, isolation and presentation choices.
    pub fn new(
        connection: ConnectionConfiguration,
        profile: ProfileId,
        presentation: Presentation,
    ) -> Self {
        Self { connection, profile, presentation }
    }
    /// Borrow the explicit connection mode and validated request.
    pub fn connection(&self) -> &ConnectionConfiguration {
        &self.connection
    }
    /// Borrow the isolated profile namespace.
    pub fn profile(&self) -> &ProfileId {
        &self.profile
    }
    /// Return the validated new-application presentation choices.
    pub fn presentation(&self) -> Presentation {
        self.presentation
    }
}

#[cfg(test)]
mod tests;
