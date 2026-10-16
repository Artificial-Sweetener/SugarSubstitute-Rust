// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain actual installed frontend files independently of disposable job observations.
//!
//! Every effect requires an admitted finite Readiness/InfrastructureLifecycle context.
//! The composing owner retains this capsule before submitting its factory. Cancellation
//! can discard a job result without disposing partial files or their original charge.
//! Attachment uses this read-only owner without acquiring backend/process authority.

#[cfg(feature = "boundary-fixtures")]
#[doc(hidden)]
pub mod boundary_fixture;
mod manifest;
#[allow(unsafe_code)]
mod native;
mod progress;
mod state;
#[cfg(test)]
mod tests;
mod types;
pub use state::{InstalledFrontendLease, InstalledFrontendOwner};
pub use types::{
    FrontendAsset, FrontendInstallationInput, FrontendSourceError, FrontendSourceManifest,
};

/// Peak known installation custody, including fixed inventories and native-path scratch.
pub const FRONTEND_SOURCE_BYTES: u64 = 32 * 1024 * 1024;
/// Maximum served runtime assets across the frontend and SugarCubes directories.
pub const MAX_FRONTEND_ASSETS: usize = 4096;
/// Maximum individual runtime asset bytes; source maps are excluded from this cohort.
pub const MAX_FRONTEND_ASSET_BYTES: u64 = 8 * 1024 * 1024;
/// Maximum aggregate expected served runtime asset bytes.
pub const MAX_FRONTEND_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
/// Fixed streamed-file and streamed-response hash scratch in bytes.
pub const FRONTEND_STREAM_BYTES: usize = 32 * 1024;
