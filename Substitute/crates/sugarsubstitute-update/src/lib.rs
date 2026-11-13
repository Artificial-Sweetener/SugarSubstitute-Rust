// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Authenticate update metadata independently of transport, native storage and scheduling.
//!
//! The pinned TUF verifier owns signatures and metadata binding. This adapter adds persistent
//! role/version floors, exact-version byte identity, bounded input and streamed payload checks.
//! Call from an admitted execution job. No method creates tasks, performs I/O or reads a clock.
mod checkpoint;
pub mod delivery;
pub mod distribution;
mod engine;
mod error;
#[cfg(any(test, feature = "boundary-fixtures"))]
#[doc(hidden)]
pub mod fixture;
pub mod http;
mod offer;
pub mod package;
mod prepared;
mod releases;
mod target;
mod trust;
pub use engine::{ENGINE_PROTOCOL, ENGINE_RELEASE};
pub use error::Error;
pub use offer::VerifiedOffer;
pub use prepared::{PreparedRelease, RECEIPT_LIFETIME_SECONDS, ReceiptRefusal};
pub use releases::{MAX_RELEASE_HISTORY_BYTES, ReleaseHistory};
pub use target::{Target, TargetVerifier, VerifiedBytes};
pub use trust::{MetadataSet, RepositoryTrust};

/// Maximum encoded bytes for one untrusted metadata role.
pub const MAX_METADATA_BYTES: usize = 1024 * 1024;
/// Maximum allowed package size, in bytes; streaming verification does not buffer this amount.
pub const MAX_TARGET_BYTES: u64 = 8 * 1024 * 1024 * 1024;
/// Maximum bytes for a protected trust checkpoint, including its root and version floors.
pub const MAX_CHECKPOINT_BYTES: usize = 8 * MAX_METADATA_BYTES;

#[cfg(test)]
mod tests;
