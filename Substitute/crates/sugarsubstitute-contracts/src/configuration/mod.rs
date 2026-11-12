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

/// Parent-authenticated backend execution policy, separate from native storage authority.
///
/// The parent derives this value from retained installation/provisioning ownership. Receiving
/// this inert value grants no directory, process or installation authority. Configuration,
/// environment, arguments and frontend presentation cannot establish installation ownership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendDataDisposition {
    /// Keep the selected external backend's ordinary bootstrap and storage semantics.
    External,
    /// Validate the preserved installation-owned model selection before executing ComfyUI.
    InstallationOwned,
}

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
            Self::validate_path(path)?;
        }
        Self::validate_arguments(arguments.iter().map(String::as_str))?;
        let port = NonZeroU16::new(port).ok_or(ConfigurationError::InvalidPort)?;
        Ok(Self { root, interpreter, arguments, port })
    }

    /// Measure actual retained path, argument-vector and string capacities in bytes.
    /// Admission consumers must charge these capacities before moving the request.
    /// Returns None if the checked total cannot be represented; fixed Self is excluded.
    pub fn allocated_bytes(&self) -> Option<usize> {
        let mut bytes = self.root.capacity().checked_add(self.interpreter.capacity())?;
        bytes = bytes.checked_add(self.arguments.capacity().checked_mul(size_of::<String>())?)?;
        for argument in &self.arguments {
            bytes = bytes.checked_add(argument.capacity())?;
        }
        Some(bytes)
    }

    /// Validate lexical absolute paths while preserving exact Windows UTF-16 units.
    /// This performs no filesystem discovery or executable authentication.
    /// # Errors
    /// Rejects ambiguous components, NUL, excessive units or invalid platform spelling.
    pub fn validate_path(path: &Path) -> Result<(), ConfigurationError> {
        if !path.is_absolute()
            || !valid_path_units(path)
            || path
                .components()
                .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
        {
            return Err(ConfigurationError::InvalidLaunchPath);
        }
        Ok(())
    }

    /// Validate exact borrowed UTF-8 arguments without copying or normalizing them.
    /// Owned and decoded request consumers share this sole argument grammar.
    /// # Errors
    /// Rejects count, individual/combined byte limits or an embedded NUL.
    pub fn validate_arguments<'a>(
        arguments: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), ConfigurationError> {
        let arguments = arguments.into_iter();
        if arguments.size_hint().0 > MAX_ARGUMENTS {
            return Err(ConfigurationError::TooManyArguments);
        }
        let mut total = 0;
        for (index, argument) in arguments.enumerate() {
            if index >= MAX_ARGUMENTS {
                return Err(ConfigurationError::TooManyArguments);
            }
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
        Ok(())
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

/// Maximum stored canvas renderer selector length in bytes.
pub const MAX_CANVAS_RENDERER_BYTES: usize = 64;

/// Stored embedded-canvas renderer preference; the renderer owner interprets stored text.
///
/// The canvas presents CPU frames unless a stored selector names an available adapter.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum CanvasRendererPreference {
    /// No preference is stored.
    #[default]
    Absent,
    /// Stored selector text of at most 64 bytes, not yet validated against any adapter.
    Stored(String),
    /// A stored value that is not bounded selector text; it is never rewritten.
    Malformed,
}

impl CanvasRendererPreference {
    /// Classify stored text, rejecting values above the selector bound.
    pub fn stored(text: &str) -> Self {
        if text.len() <= MAX_CANVAS_RENDERER_BYTES {
            Self::Stored(text.to_owned())
        } else {
            Self::Malformed
        }
    }
}

/// Validated startup intent containing no live resources or native privileges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartupConfiguration {
    connection: ConnectionConfiguration,
    profile: ProfileId,
    presentation: Presentation,
    canvas_renderer: CanvasRendererPreference,
}

impl StartupConfiguration {
    /// Compose already validated connection, isolation and presentation choices.
    /// No canvas renderer preference is stored until [`Self::with_canvas_renderer`].
    pub fn new(
        connection: ConnectionConfiguration,
        profile: ProfileId,
        presentation: Presentation,
    ) -> Self {
        Self {
            connection,
            profile,
            presentation,
            canvas_renderer: CanvasRendererPreference::Absent,
        }
    }
    /// Attach the stored embedded-canvas renderer preference.
    #[must_use]
    pub fn with_canvas_renderer(mut self, preference: CanvasRendererPreference) -> Self {
        self.canvas_renderer = preference;
        self
    }
    /// Borrow the stored embedded-canvas renderer preference.
    pub fn canvas_renderer(&self) -> &CanvasRendererPreference {
        &self.canvas_renderer
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

#[cfg(target_os = "windows")]
fn valid_path_units(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().enumerate().all(|(index, unit)| index < 32766 && unit != 0)
}

#[cfg(not(target_os = "windows"))]
fn valid_path_units(path: &Path) -> bool {
    path.to_str().is_some_and(|text| !text.contains('\0') && text.encode_utf16().count() <= 32766)
}

#[cfg(test)]
mod tests;
