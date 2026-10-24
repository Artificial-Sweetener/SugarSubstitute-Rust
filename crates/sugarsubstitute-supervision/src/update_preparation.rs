// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare an existing installation through owned execution jobs without taking its run lock.
//!
//! The host admits one step at a time and observes completion before submitting the next.
//! Cancellation retains custody until an explicit retirement job finishes. No step starts a
//! process, activates a candidate, provisions ComfyUI, or changes the user's subscription.
mod state;
mod worker;
use crate::setup_storage::{self, PreparationImage, PreparedUpdater, SignedCandidate};
use std::{path::PathBuf, time::Duration};
use sugarsubstitute_contracts::update::{Admission, subscription::Subscription};
pub use sugarsubstitute_update::distribution::Distribution;
use sugarsubstitute_update::http::Policy;
pub use worker::{Observation, Worker};

/// Locally selected installation, subscription and transport policy, fixed for this campaign.
pub struct Configuration {
    /// Existing native installation whose protected trust was established by its installer.
    pub installation: PathBuf,
    /// Owned extraction workspace; the storage adapter validates its resolved ownership.
    pub workspace: PathBuf,
    /// Distribution-owned repository identity, stable across signing-key rotation.
    pub repository: [u8; 32],
    /// Distribution-owned allowed origins, never populated from remote metadata.
    pub policy: Policy,
    /// Finite idle network timeout, independent of the host's per-job execution deadline.
    pub idle: Duration,
}

/// Local startup inputs. Repository authority comes from protected installation state;
/// this configuration cannot supply a transport or initialize new trust.
pub struct CachedConfiguration {
    /// Existing native installation with established trust continuity.
    pub installation: PathBuf,
    /// Owned extraction workspace for repairing authenticated cached preparation.
    pub workspace: PathBuf,
}

/// Authenticated engine custody for the process owner; preparation alone never proves health.
pub enum EngineUpdate {
    /// Newly staged engine requiring a supervised probe before acceptance or use.
    Prepared(Box<PreparedUpdater>),
    /// Healthy installed engine, independently reverified and pinned for a preparation helper.
    Accepted(Box<PreparationImage>),
}

/// Last completed boundary; download progress counts durable bytes against an authenticated total.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Progress {
    /// Open the existing protected repository binding.
    Opening,
    /// Authenticate repository metadata, continuing bounded root rotation when needed.
    Checking,
    /// Reverify an already accepted engine against the application's signed minimum requirement.
    SelectingEngine,
    /// Authenticate the updater target using this process's actual compiled capabilities.
    CheckingEngine,
    /// Reauthenticate saved signed metadata without network access.
    Restoring,
    /// Verify a completed generation's protected publication, payloads and saved configuration.
    Reopening,
    /// Save newly authenticated restart evidence after completed candidate custody is retained.
    Remembering,
    /// Authenticate complete archive bytes needed to repair unfinished or missing preparation.
    RestoringArchive,
    /// Request another package interval, retaining already flushed bytes on disk.
    Downloading {
        /// Package bytes durably received at the last completed interval.
        received: u64,
        /// Authenticated archive length in bytes.
        total: u64,
    },
    /// Reopen verified preparation or extract and stage repair, preserving saved ComfyUI choices.
    Preparing,
    /// Extract and stage an independent updater, preserving application selection and ComfyUI.
    PreparingEngine,
    /// Candidate is staged; activation still requires independent run exclusion and health.
    Ready,
    /// Engine custody is ready to transfer to its probe or preparation process owner.
    EngineReady,
    /// Cache-only startup has pinned an accepted engine with an explicit activation entry.
    ActivationReady,
    /// Another native preparation owner is live; this campaign did not start network work.
    /// This is a terminal coalescing result, not an up-to-date or completed-update assertion.
    AlreadyPreparing,
    /// Discovery requires no package download or a different update engine/installer.
    Decision(Admission),
    /// No complete cached update is available; this does not assert that the app is current.
    Unavailable,
    /// A step failed; its cause is retained until the campaign is retired.
    Failed,
    /// Cancellation has been requested; resource retirement is still outstanding.
    Retiring,
    /// All campaign custody has retired, or its candidate was explicitly transferred.
    Closed,
}
impl Progress {
    /// Peak new admission for this step, in addition to custody retained by earlier jobs.
    fn reservation(self) -> u64 {
        match self {
            Self::Opening => setup_storage::TRUST_RESERVATION_BYTES,
            Self::Checking | Self::CheckingEngine => {
                setup_storage::OFFER_RESERVATION_BYTES
                    + setup_storage::TRUST_OPERATION_RESERVATION_BYTES
                    + crate::update_http::RESERVATION_BYTES
            }
            Self::Downloading { .. } => {
                setup_storage::DOWNLOAD_RESERVATION_BYTES
                    + setup_storage::TRUST_OPERATION_RESERVATION_BYTES
                    + crate::update_http::RESERVATION_BYTES
            }
            Self::Preparing => {
                2 * setup_storage::RESERVATION_BYTES
                    + setup_storage::TRUST_OPERATION_RESERVATION_BYTES
                    + setup_storage::OFFER_PUBLICATION_RESERVATION_BYTES
            }
            Self::SelectingEngine => {
                setup_storage::RESERVATION_BYTES + setup_storage::TRUST_OPERATION_RESERVATION_BYTES
            }
            Self::PreparingEngine => {
                setup_storage::RESERVATION_BYTES + setup_storage::UPDATER_STAGING_RESERVATION_BYTES
            }
            Self::Restoring => {
                setup_storage::OFFER_LOOKUP_RESERVATION_BYTES
                    + setup_storage::TRUST_OPERATION_RESERVATION_BYTES
            }
            Self::Reopening => {
                setup_storage::LAUNCH_RESERVATION_BYTES
                    + setup_storage::TRUST_OPERATION_RESERVATION_BYTES
            }
            Self::Remembering => {
                setup_storage::TRUST_OPERATION_RESERVATION_BYTES
                    + setup_storage::OFFER_PUBLICATION_RESERVATION_BYTES
            }
            Self::RestoringArchive => {
                setup_storage::DOWNLOAD_RESERVATION_BYTES
                    + setup_storage::TRUST_OPERATION_RESERVATION_BYTES
            }
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests;
