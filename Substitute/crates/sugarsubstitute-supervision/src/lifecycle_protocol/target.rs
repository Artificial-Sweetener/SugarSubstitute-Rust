// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow supported managed inputs; explicit conversion grants no filesystem trust.
use super::{ArgumentRefs, ProtocolError, Utf16Text};
use std::{num::NonZeroU16, path::PathBuf};
use sugarsubstitute_contracts::{
    configuration::{ConnectionConfiguration, LaunchConfiguration, LoopbackHost, LoopbackOrigin},
    lifecycle::BackendTarget,
};

/// Borrow the already validated configured or acquired backend without cloning its storage.
/// This inert view grants no listener, process, readiness or filesystem authority.
#[derive(Clone, Copy, Debug)]
pub enum BorrowedBackendTarget<'a> {
    /// Borrow the exact validated managed launch paths and argument allocations.
    Managed(&'a LaunchConfiguration),
    /// Copy the fixed-size validated loopback endpoint only.
    Attached(LoopbackOrigin),
}
impl<'a> From<&'a BackendTarget> for BorrowedBackendTarget<'a> {
    fn from(value: &'a BackendTarget) -> Self {
        match value {
            BackendTarget::Managed(value) => Self::Managed(value),
            BackendTarget::Attached(value) => Self::Attached(*value),
        }
    }
}
impl<'a> From<&'a ConnectionConfiguration> for BorrowedBackendTarget<'a> {
    fn from(value: &'a ConnectionConfiguration) -> Self {
        match value {
            ConnectionConfiguration::Launch(value) => Self::Managed(value),
            ConnectionConfiguration::Attach(value) => Self::Attached(*value),
        }
    }
}

/// Preflighted Unicode launch data awaiting allocation admission and domain validation.
#[derive(Clone, Copy, Debug)]
pub struct ManagedTargetRef<'a> {
    pub(super) root: Utf16Text<'a>,
    pub(super) interpreter: Utf16Text<'a>,
    pub(super) arguments: ArgumentRefs<'a>,
    pub(super) port: NonZeroU16,
}
impl<'a> ManagedTargetRef<'a> {
    /// Borrow the exact requested root; canonical trust remains with the native owner.
    pub const fn root(self) -> Utf16Text<'a> {
        self.root
    }
    /// Borrow the exact requested interpreter path without normalization.
    pub const fn interpreter(self) -> Utf16Text<'a> {
        self.interpreter
    }
    /// Borrow the original argv list without joining it into shell text.
    pub const fn arguments(self) -> ArgumentRefs<'a> {
        self.arguments
    }
    /// Return the requested nonzero backend port.
    pub const fn port(self) -> NonZeroU16 {
        self.port
    }
}
/// Borrow a target from request bytes, never from authenticated native observations.
#[derive(Clone, Copy, Debug)]
pub enum DecodedTarget<'a> {
    /// Strict UTF-16 launch data with complete count/UTF-8-budget preflight.
    Managed(ManagedTargetRef<'a>),
    /// Enumerated loopback host and nonzero port; no arbitrary URL is accepted.
    Attached {
        /// Exact admitted host spelling.
        host: LoopbackHost,
        /// Exact nonzero backend port.
        port: NonZeroU16,
    },
}
impl DecodedTarget<'_> {
    /// Explicitly allocate after admission and apply the existing domain constructor.
    /// Counts, lengths, UTF-16 and complete UTF-8 argv budgets have already passed
    /// before any allocation. Absolute-path and dot-component rules remain owned
    /// by `LaunchConfiguration`; this conversion performs no canonical resolution.
    pub fn to_domain(self) -> Result<BackendTarget, ProtocolError> {
        match self {
            Self::Managed(target) => {
                let root = PathBuf::from(target.root.to_owned_string()?);
                let interpreter = PathBuf::from(target.interpreter.to_owned_string()?);
                let arguments = target.arguments.owned()?;
                LaunchConfiguration::new(root, interpreter, arguments, target.port.get())
                    .map(BackendTarget::Managed)
                    .map_err(ProtocolError::Configuration)
            }
            Self::Attached { host, port } => {
                let host = match host {
                    LoopbackHost::Ipv4 => "127.0.0.1",
                    LoopbackHost::Ipv6 => "[::1]",
                    LoopbackHost::Localhost => "localhost",
                };
                LoopbackOrigin::parse(&format!("http://{host}:{}", port.get()), port.get())
                    .map(BackendTarget::Attached)
                    .map_err(ProtocolError::Configuration)
            }
        }
    }
}
