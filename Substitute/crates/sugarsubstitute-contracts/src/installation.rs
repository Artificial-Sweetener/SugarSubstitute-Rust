// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Durable native installation selection, independent of filesystem and process adapters.
//!
//! Transitions return a proposed replacement record. An adapter must serialize writers,
//! compare the original revision, and durably publish the replacement before acting on it.
//! Decoding a record proves neither package authenticity nor process ownership. In
//! particular, a persisted trial requires live-owner reconciliation before recovery.
mod identity;
mod record;
mod transition;
mod verdict;
pub use identity::{GenerationId, InstallationId, PackageDigest};
pub use record::{
    Candidate, Generation, InstallationRecord, SignedRelease, Strike, Trial, Unwitnessed,
};
pub use verdict::{
    BadTrial, CloseCause, FailedProcess, HostExit, TrialFacts, TrialVerdict,
    UNRESPONSIVE_EXIT_STATUS, Undecided,
};

/// Selection schema read and written by this installation state owner.
pub const SCHEMA_VERSION: u32 = 3;

/// Maximum encoded installation selection size in bytes, checked before parsing.
pub const MAX_RECORD_BYTES: usize = 16 * 1024;
/// Bounded recently struck package identities retained to suppress automatic retry loops.
pub const MAX_STRUCK_PACKAGES: usize = 16;
/// Unexpired strikes that withhold a package from automatic preparation.
pub const QUARANTINE_STRIKES: u8 = 2;
/// Seconds a strike keeps counting, and so how long a withheld package stays withheld.
pub const STRIKE_LIFETIME_SECONDS: u64 = 7 * 24 * 60 * 60;
/// Abandoned trials of one package in a row whose last one counts as a strike.
pub const UNWITNESSED_LIMIT: u8 = 3;

/// A record or transition that cannot be admitted without changing its meaning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rejection {
    /// An identifier is not in its canonical bounded spelling.
    Identity,
    /// The record cannot be decoded or contains unknown fields.
    Encoding,
    /// The schema has no supported reader; preserve its original bytes.
    Schema,
    /// The record exceeds a fixed admission bound or its revision is exhausted.
    Capacity,
    /// Accepted, retained, or candidate references contradict each other.
    Invariant,
    /// The requested operation is not valid in this transaction phase.
    Phase,
    /// A completion belongs to a different installation, generation or attempt.
    Binding,
    /// Automatic retry of a package this installation currently withholds after bad trials.
    Quarantined,
}
impl std::fmt::Display for Rejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Installation contract rejected: {self:?}")
    }
}
impl std::error::Error for Rejection {}

/// Distinguish an automatic offer from an explicit user-requested repair retry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparationIntent {
    /// Do not automatically retry a known failed package.
    Automatic,
    /// The user explicitly requested another attempt using this verified package.
    Repair,
}

/// Required startup action; a recorded trial is never assumed to be a dead process.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupSelection<'a> {
    /// Start the accepted generation; preparation, if any, can continue independently.
    Accepted(&'a Generation),
    /// No healthy generation exists; setup or repair must finish before normal launch.
    NeedsInstallation,
    /// Reconcile the exact attempt with native ownership before joining or rejecting it.
    ReconcileTrial(&'a Trial),
}

#[cfg(test)]
mod tests;
