// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Validate the first host's bounded, versioned frontend readiness receipt.

use crate::execution::{Generation, IdentityError};
use serde::Deserialize;
use std::{error::Error, fmt, num::NonZeroU64};

/// Identify one application-owned launch attempt independently of navigation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LaunchId(NonZeroU64);

impl LaunchId {
    /// Construct a positive launch identity.
    ///
    /// # Errors
    /// Rejects zero, which cannot identify an application launch.
    pub const fn new(value: u64) -> Result<Self, IdentityError> {
        match NonZeroU64::new(value) {
            Some(value) => Ok(Self(value)),
            None => Err(IdentityError::Zero),
        }
    }

    /// Return the launch value without granting process ownership.
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Retain separately observed frontend and installed-extension evidence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FrontendEvidence {
    /// Served ComfyUI frontend version, bounded to 64 ASCII version characters.
    pub frontend_version: String,
    /// Installed SugarCubes version under the same bound.
    pub sugarcubes_version: String,
    /// The supported frontend application and its live graph refer to the same owner.
    pub app_graph_ready: bool,
    /// The actual graph canvas is connected with positive visible bounds.
    pub canvas_connected: bool,
    /// Frontend loading overlays and workspace spinner have finished.
    pub loading_finished: bool,
    /// SugarCubes completed its supported extension registration.
    pub sugarcubes_registered: bool,
    /// SugarCubes reports ready without an extension setup failure.
    pub sugarcubes_ready: bool,
}

/// Classify a terminal web probe failure without embedding native commands.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessFailure {
    /// The actual frontend application or graph could not become usable.
    FrontendUnavailable,
    /// The required installed extension could not be observed.
    SugarcubesUnavailable,
    /// The installed extension reported setup failure.
    SugarcubesFailed,
    /// The probe's application-owned deadline expired.
    TimedOut,
}

/// Preserve a bounded untrusted probe diagnostic for explicit reporting.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProbeFailure {
    /// Stable classification selected by the probe.
    pub code: ReadinessFailure,
    /// At most 1024 UTF-8 bytes; never automatically logged or treated as product copy.
    pub detail: String,
}

/// Carry one terminal probe result for presentation readiness.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReadinessOutcome {
    /// Both observed owners are usable in the current frontend.
    Ready {
        /// Evidence of completed frontend and extension setup.
        evidence: FrontendEvidence,
    },
    /// The probe ended with its original bounded diagnostic.
    Failed {
        /// Failure classification and observed detail.
        failure: ProbeFailure,
    },
}

/// Own a fully validated receipt whose identity and result cannot be mutated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadinessReceipt {
    launch: LaunchId,
    navigation: Generation,
    outcome: ReadinessOutcome,
}

impl ReadinessReceipt {
    /// Current wire schema accepted by this first-checkpoint host.
    pub const SCHEMA_VERSION: u32 = 1;
    /// Maximum received UTF-8 JSON length, checked before allocation or parsing.
    pub const MAX_BYTES: usize = 4096;

    /// Parse a terminal receipt after the desktop adapter validates message origin.
    ///
    /// # Errors
    /// Rejects oversized, malformed, unknown-version or unknown-field input,
    /// nonpositive identities, incomplete readiness and invalid bounded strings.
    /// JSON errors preserve their original cause without logging payload bytes.
    pub fn parse(bytes: &[u8]) -> Result<Self, ReadinessError> {
        if bytes.len() > Self::MAX_BYTES {
            return Err(ReadinessError::TooLarge);
        }
        let wire: WireReceipt = serde_json::from_slice(bytes).map_err(ReadinessError::Json)?;
        if wire.schema_version != Self::SCHEMA_VERSION {
            return Err(ReadinessError::UnsupportedVersion(wire.schema_version));
        }
        let launch = LaunchId::new(wire.launch_id).map_err(ReadinessError::Identity)?;
        let navigation =
            Generation::new(wire.navigation_generation).map_err(ReadinessError::Identity)?;
        match &wire.outcome {
            ReadinessOutcome::Ready { evidence } => {
                if !version(&evidence.frontend_version) || !version(&evidence.sugarcubes_version) {
                    return Err(ReadinessError::InvalidVersion);
                }
                if !(evidence.app_graph_ready
                    && evidence.canvas_connected
                    && evidence.loading_finished
                    && evidence.sugarcubes_registered
                    && evidence.sugarcubes_ready)
                {
                    return Err(ReadinessError::IncompleteEvidence);
                }
            }
            ReadinessOutcome::Failed { failure } => {
                if failure.detail.len() > 1024 || failure.detail.contains('\0') {
                    return Err(ReadinessError::InvalidFailureDetail);
                }
            }
        }
        Ok(Self { launch, navigation, outcome: wire.outcome })
    }

    /// Return the application launch attested by this web presentation receipt.
    pub const fn launch(&self) -> LaunchId {
        self.launch
    }

    /// Return the positive navigation generation observed by the probe.
    pub const fn navigation(&self) -> Generation {
        self.navigation
    }

    /// Borrow the immutable terminal evidence without granting native authority.
    pub const fn outcome(&self) -> &ReadinessOutcome {
        &self.outcome
    }

    /// Transfer terminal evidence after application identity and admission checks.
    pub fn into_outcome(self) -> ReadinessOutcome {
        self.outcome
    }
}

/// Keep serialized fields separate from validated application contracts.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireReceipt {
    schema_version: u32,
    launch_id: u64,
    navigation_generation: u64,
    outcome: ReadinessOutcome,
}

/// Reject unbounded or control-bearing observed version labels.
fn version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || b".+-_".contains(&byte))
}

/// Distinguish malformed protocol data from incomplete presentation evidence.
#[derive(Debug)]
pub enum ReadinessError {
    /// The incoming JSON exceeds four KiB.
    TooLarge,
    /// Parsing failed, preserving the original cause.
    Json(serde_json::Error),
    /// This host accepts only its explicitly registered schema.
    UnsupportedVersion(u32),
    /// Launch or navigation identity was zero.
    Identity(IdentityError),
    /// The receipt claims readiness before all required observations succeeded.
    IncompleteEvidence,
    /// An observed version is missing or violates its bounded character contract.
    InvalidVersion,
    /// Failure detail exceeds its bound or contains a NUL character.
    InvalidFailureDetail,
}

impl fmt::Display for ReadinessError {
    /// Describe the protocol rejection without exposing untrusted payload bytes.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => formatter.write_str("Readiness receipt exceeds 4096 bytes"),
            Self::Json(error) => write!(
                formatter,
                "Invalid readiness JSON ({:?}) at line {}, column {}",
                error.classify(),
                error.line(),
                error.column()
            ),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "Unsupported readiness schema {version}")
            }
            Self::Identity(error) => error.fmt(formatter),
            Self::IncompleteEvidence => {
                formatter.write_str("Frontend readiness evidence is incomplete")
            }
            Self::InvalidVersion => {
                formatter.write_str("Invalid observed frontend or extension version")
            }
            Self::InvalidFailureDetail => formatter.write_str("Invalid readiness failure detail"),
        }
    }
}

impl Error for ReadinessError {
    /// Preserve parse and identity causes across the application boundary.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::Identity(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "readiness_tests.rs"]
mod tests;
