// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read-only view of the selection record for update presentation.
//!
//! The view grants no launch, preparation or activation authority. It is a consistent value at
//! the moment of the shared record read and may be stale as soon as it is returned.
use super::*;
use sugarsubstitute_contracts::installation::{Generation, GenerationId};

/// Selection state an update surface shows: what runs, what activates next and what was
/// rolled back.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateSelection {
    /// The accepted generation, which every ordinary launch starts.
    pub running: InstalledRelease,
    /// A completely prepared candidate that activates at the next launch. Staging that is
    /// still incomplete, a trial in progress, and a rejected or superseded candidate are not
    /// prepared. A trial that ends without a verdict returns to prepared, so this is `Some`
    /// again after such a trial.
    pub prepared: Option<PreparedUpdate>,
    /// Set only after a newer generation's trial was judged bad and the running generation
    /// stayed accepted in its place; a trial that ends without a verdict never sets it.
    /// `None` again once a newer release is accepted.
    pub rollback: Option<Rollback>,
    /// The newer release whose files were found damaged, so the running generation started in
    /// its place while an update check downloads it again. This is not a bad verdict. Shown as
    /// "SugarSubstitute is running the previous version while it repairs the update."
    /// `None` again once that release is accepted.
    pub repairing: Option<InstalledRelease>,
}

/// An installed generation and the signed offer label it was prepared from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledRelease {
    /// Opaque generation identity.
    pub generation: GenerationId,
    /// The signed offer's `version()` label.
    pub version: String,
    /// The signed offer's release sequence.
    pub release_sequence: u64,
}

/// A prepared candidate waiting for the next launch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedUpdate {
    /// Opaque generation identity.
    pub generation: GenerationId,
    /// The signed offer's `version()` label.
    pub version: String,
    /// The signed offer's release sequence.
    pub release_sequence: u64,
    /// SHA-256 of the authenticated package manifest the candidate was verified against.
    /// This is not the update archive digest; compare offers by `release_sequence`.
    pub package_digest: [u8; 32],
}

/// The newer generation whose trial was judged bad.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rollback {
    /// Opaque identity of the rejected generation.
    pub rejected_generation: GenerationId,
    /// The rejected offer's `version()` label.
    pub rejected_version: String,
    /// The rejected offer's release sequence.
    pub rejected_release_sequence: u64,
}

/// Read a snapshot of the selection record for update presentation.
///
/// Call this on an admitted blocking storage job. It takes only the shared record read, never
/// the selection writer or any run custody, so it neither waits for an application run nor
/// delays one. It never enables a namespace that has not been enabled yet.
/// # Errors
/// `Missing` when the installation has no enabled selection or no accepted generation yet,
/// `LockTimeout` or `LockCancelled` when a publication holds the record past the bounded wait,
/// the deadline or cancellation of `context`, and `Storage` for damaged state, whose bytes are
/// preserved.
pub fn read_update_selection(
    installation: &Path,
    context: &JobContext,
) -> Result<UpdateSelection, SelectionFailure> {
    let store =
        SelectionStore::existing(installation, context)?.ok_or(SelectionFailure::Missing)?;
    let record = store.load(context)?.ok_or(SelectionFailure::Missing)?;
    UpdateSelection::of(&record)
}

impl UpdateSelection {
    /// Project a validated record; only `Candidate::Prepared` is a prepared update.
    pub(super) fn of(record: &InstallationRecord) -> Result<Self, SelectionFailure> {
        let running = record.accepted().ok_or(SelectionFailure::Missing)?;
        let prepared = match record.candidate() {
            Some(Candidate::Prepared { generation }) => Some(PreparedUpdate {
                generation: generation.id.clone(),
                version: generation.release.version().to_owned(),
                release_sequence: generation.release.sequence(),
                package_digest: digest_bytes(generation)?,
            }),
            _ => None,
        };
        Ok(Self {
            running: InstalledRelease {
                generation: running.id.clone(),
                version: running.release.version().to_owned(),
                release_sequence: running.release.sequence(),
            },
            prepared,
            rollback: record.rollback().map(|rejected| Rollback {
                rejected_generation: rejected.id.clone(),
                rejected_version: rejected.release.version().to_owned(),
                rejected_release_sequence: rejected.release.sequence(),
            }),
            repairing: record.repair().map(|damaged| InstalledRelease {
                generation: damaged.id.clone(),
                version: damaged.release.version().to_owned(),
                release_sequence: damaged.release.sequence(),
            }),
        })
    }
}

/// Decode the canonical lowercase hexadecimal manifest digest.
fn digest_bytes(generation: &Generation) -> Result<[u8; 32], SelectionFailure> {
    let text = generation.package.as_str().as_bytes();
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let pair =
            std::str::from_utf8(&text[index * 2..index * 2 + 2]).map_err(io::Error::other)?;
        *byte = u8::from_str_radix(pair, 16).map_err(io::Error::other)?;
    }
    Ok(bytes)
}
