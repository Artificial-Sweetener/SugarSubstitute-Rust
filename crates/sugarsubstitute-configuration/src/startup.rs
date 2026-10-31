// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Parse bounded schema-one startup inputs without acquiring filesystem or process authority.
//!
//! The caller owns reading `startup.local.toml` through a governed startup job.
//! This adapter parses borrowed bytes only; launch paths still require canonical
//! filesystem trust validation by the resource owner before use.

use serde::Deserialize;
use std::{error::Error, fmt, path::PathBuf, str::Utf8Error};
use sugarsubstitute_contracts::configuration::{
    CanvasRendererPreference, ConfigurationError, ConnectionConfiguration, LaunchConfiguration,
    LoopbackOrigin, Presentation, ProfileId, StartupConfiguration, Theme,
};

/// Maximum UTF-8 document size admitted before decoding or TOML parsing.
pub const MAX_STARTUP_CONFIGURATION_BYTES: usize = 64 * 1024;

/// A bounded startup document could not produce validated startup intent.
pub enum StartupConfigurationError {
    /// The document exceeds the byte limit.
    TooLarge,
    /// The document is not valid UTF-8; the original decoding cause is retained.
    Utf8(Utf8Error),
    /// TOML syntax or its explicit schema is invalid; the original cause is retained.
    Syntax(toml::de::Error),
    /// The document declares an unsupported schema version, or a field newer than its version.
    UnsupportedSchema,
    /// Exactly one table matching the explicit mode must be present.
    ModeTables,
    /// A syntactically valid field failed domain validation.
    Invalid(ConfigurationError),
}

impl fmt::Display for StartupConfigurationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TooLarge => "Startup configuration exceeds 65536 bytes",
            Self::Utf8(_) => "Startup configuration must be valid UTF-8",
            Self::Syntax(_) => "Startup configuration has invalid TOML or schema fields",
            Self::UnsupportedSchema => "Startup configuration requires schema version 1 or 2",
            Self::ModeTables => {
                "Startup configuration requires exactly one table matching its mode"
            }
            Self::Invalid(_) => "Startup configuration contains an invalid setting",
        })
    }
}

impl fmt::Debug for StartupConfigurationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl Error for StartupConfigurationError {
    /// Preserve original causes for explicit inspection, independently of redacted diagnostics.
    ///
    /// TOML causes can contain supplied source text. Runtime logging must use the
    /// redacted outer error rather than formatting this source chain.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Utf8(cause) => Some(cause),
            Self::Syntax(cause) => Some(cause),
            Self::Invalid(cause) => Some(cause),
            _ => None,
        }
    }
}

/// Parse schema-one or schema-two configuration with explicit connection and presentation fields.
///
/// Schema two adds the optional `canvas_renderer` preference; a schema-one document reads as
/// schema two without a preference, and its bytes are never rewritten. A stored preference that
/// is not bounded text is an observable [`CanvasRendererPreference::Malformed`] miss rather than
/// a startup failure. Rejects oversized/non-UTF-8 bytes, malformed or duplicate TOML, unknown
/// fields, unsupported versions, absent/mixed mode tables and invalid domain values.
/// Arguments retain their exact Unicode spelling. Errors display no supplied
/// paths, arguments, credentials or untrusted field values; original parsing
/// causes remain available through [`Error::source`] for deliberate inspection.
pub fn parse_startup_configuration(
    bytes: &[u8],
) -> Result<StartupConfiguration, StartupConfigurationError> {
    if bytes.len() > MAX_STARTUP_CONFIGURATION_BYTES {
        return Err(StartupConfigurationError::TooLarge);
    }
    let text = std::str::from_utf8(bytes).map_err(StartupConfigurationError::Utf8)?;
    let document: Document = toml::from_str(text).map_err(StartupConfigurationError::Syntax)?;
    let canvas_renderer = match (document.schema_version, document.canvas_renderer) {
        (1, None) | (2, None) => CanvasRendererPreference::Absent,
        (2, Some(toml::Value::String(text))) => CanvasRendererPreference::stored(&text),
        (2, Some(_)) => CanvasRendererPreference::Malformed,
        _ => return Err(StartupConfigurationError::UnsupportedSchema),
    };
    let connection = match (document.mode, document.launch, document.attach) {
        (Mode::Launch, Some(launch), None) => ConnectionConfiguration::Launch(
            LaunchConfiguration::new(
                PathBuf::from(launch.root),
                PathBuf::from(launch.interpreter),
                launch.arguments,
                document.port,
            )
            .map_err(StartupConfigurationError::Invalid)?,
        ),
        (Mode::Attach, None, Some(attach)) => ConnectionConfiguration::Attach(
            LoopbackOrigin::parse(&attach.origin, document.port)
                .map_err(StartupConfigurationError::Invalid)?,
        ),
        _ => return Err(StartupConfigurationError::ModeTables),
    };
    let profile = ProfileId::new(document.profile).map_err(StartupConfigurationError::Invalid)?;
    let theme = match document.theme {
        WireTheme::Dark => Theme::Dark,
        WireTheme::Light => Theme::Light,
    };
    let presentation = Presentation::new(&document.locale, theme, document.reduced_motion)
        .map_err(StartupConfigurationError::Invalid)?;
    Ok(StartupConfiguration::new(connection, profile, presentation)
        .with_canvas_renderer(canvas_renderer))
}

/// Keep the persisted schema independent of validated domain construction.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    schema_version: u32,
    mode: Mode,
    port: u16,
    profile: String,
    locale: String,
    theme: WireTheme,
    reduced_motion: bool,
    launch: Option<Launch>,
    attach: Option<Attach>,
    /// Schema-two embedded canvas renderer preference; any stored value shape is accepted here.
    canvas_renderer: Option<toml::Value>,
}

/// Require an explicit persisted connection mode.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Mode {
    Launch,
    Attach,
}

/// Decode exact appearance spellings before constructing the shared theme.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum WireTheme {
    Dark,
    Light,
}

/// Decode only launch-owned fields; arbitrary command strings are not supported.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Launch {
    root: String,
    interpreter: String,
    arguments: Vec<String>,
}

/// Decode only the external root origin.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Attach {
    origin: String,
}

#[cfg(test)]
#[path = "startup_tests.rs"]
mod tests;
