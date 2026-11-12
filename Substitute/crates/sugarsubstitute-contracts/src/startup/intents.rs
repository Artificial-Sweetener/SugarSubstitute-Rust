// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Separate user cancellation from normal splash retirement and native authority.

use super::LaunchId;
use crate::{configuration::LoopbackOrigin, execution::Generation};

/// Express the first host's startup intent; output disclosure stays with the view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplashIntent {
    /// Cancel loading and permanently close this attempt's readiness admission.
    Cancel,
}

/// Select cleanup semantics from the configured connection mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionCleanup {
    /// Cancel managed preparation and release only resources owned by the attempt.
    /// This is a request to the supervisor, not permission to terminate a PID.
    CancelManagedPreparation,
    /// Disconnect an attachment without stopping the externally owned backend.
    DisconnectAttachment,
}

/// Emit at most one concrete effect per application transition.
///
/// The host executes these through authoritative adapters. Returning an effect
/// does not mean it ran or settled. The connection request is borrowed from the
/// session's validated configuration rather than duplicated in the effect.
/// These in-process values are not a persisted or authenticated host protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupEffect {
    /// Begin the configured connection only after successful native splash submission.
    PrepareConnection {
        /// Application attempt identity required on subsequent observations.
        launch: LaunchId,
    },
    /// Start the current frontend navigation after backend readiness.
    LoadFrontend {
        /// Application attempt identity.
        launch: LaunchId,
        /// Sole current navigation eligible for frontend completion.
        navigation: Generation,
        /// Admitted endpoint, without process ownership or arbitrary URL data.
        origin: LoopbackOrigin,
    },
    /// Retire the splash normally while preserving the ready connection.
    RetireSplash {
        /// Application attempt whose actual retirement must be acknowledged.
        launch: LaunchId,
    },
    /// Reveal the usable main surface after actual splash retirement.
    RevealMain {
        /// Application attempt whose frontend and handoff were acknowledged.
        launch: LaunchId,
    },
    /// Request mode-appropriate cleanup; wait for observed settlement before retry.
    CleanupConnection {
        /// Attempt whose owned preparation or attachment must settle.
        launch: LaunchId,
        /// Cleanup policy, never a process handle or permission to stop external work.
        mode: ConnectionCleanup,
    },
}
