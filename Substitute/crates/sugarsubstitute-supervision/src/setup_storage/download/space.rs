// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check the free space an update needs before writing it, and reclaim what the runtime cache
//! can give back before reporting a shortage.
//!
//! A shortage is typed ([`SpaceShortage`]) so callers can tell a full disk from every other
//! storage failure: it is not transient, it is not retried within an attempt, and the update
//! report names it. The cache's own logical budget is a separate condition
//! ([`super::super::cache::CacheBudgetExhausted`]) and never reads as a full disk.
use super::super::{cache::RuntimeCache, check};
use std::{error::Error, fmt, io, path::Path};
use sugarsubstitute_contracts::execution::JobContext;

/// Bytes an update always leaves free on the volume, so it never fills the user's disk to the
/// last byte and the system and other applications keep room to work.
pub const HEADROOM_BYTES: u64 = 256 * 1024 * 1024;

/// The volume holding a directory does not have the space an update step needs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpaceShortage {
    /// Bytes the step still has to write, plus [`HEADROOM_BYTES`].
    pub needed: u64,
    /// Bytes the volume offered this user when measured, after reclaiming.
    pub available: u64,
}
impl fmt::Display for SpaceShortage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Not enough free disk space for the update: {} MB needed, {} MB free",
            self.needed.div_ceil(1024 * 1024),
            self.available / (1024 * 1024)
        )
    }
}
impl Error for SpaceShortage {}
impl SpaceShortage {
    /// This shortage as an I/O error of kind `StorageFull` whose cause is the shortage itself,
    /// for owners whose results are I/O errors.
    pub fn into_io(self) -> io::Error {
        io::Error::new(io::ErrorKind::StorageFull, self)
    }

    /// The shortage `cause` carries anywhere in its chain.
    pub fn find(cause: &(dyn Error + 'static)) -> Option<Self> {
        let mut current = Some(cause);
        while let Some(error) = current {
            if let Some(shortage) = error.downcast_ref::<Self>() {
                return Some(*shortage);
            }
            if let Some(inner) = error.downcast_ref::<io::Error>().and_then(io::Error::get_ref) {
                current = Some(inner);
                continue;
            }
            current = error.source();
        }
        None
    }
}

/// Measures the bytes this user may still write on the volume holding a directory.
pub trait FreeSpace {
    /// Free bytes available to the caller on the volume holding `directory`.
    /// # Errors
    /// Returns the native query failure.
    fn available(&self, directory: &Path) -> io::Result<u64>;
}

/// The operating system's own per-user free-space answer, which honours disk quotas.
pub struct VolumeSpace;
impl FreeSpace for VolumeSpace {
    #[allow(unsafe_code)]
    fn available(&self, directory: &Path) -> io::Result<u64> {
        use windows::{Win32::Storage::FileSystem::GetDiskFreeSpaceExW, core::HSTRING};
        let name = HSTRING::from(directory);
        let mut available = 0_u64;
        // SAFETY: `name` is a NUL-terminated wide string that outlives this synchronous call,
        // and `available` is a live, writable u64 the call fills before returning. The two
        // optional totals are not requested, and nothing is retained after return.
        unsafe { GetDiskFreeSpaceExW(&name, Some(&raw mut available), None, None) }
            .map_err(io::Error::other)?;
        Ok(available)
    }
}

/// Gives disk space back before a shortage is reported.
pub trait Reclaim {
    /// Remove what can be removed safely now. Content in use, unknown content and
    /// authoritative data are never removed.
    /// # Errors
    /// Returns the owner's cancellation, contention or storage failure.
    fn reclaim(&mut self, context: &JobContext) -> io::Result<()>;
}

/// Runtime cache maintenance: idle and expired entries go, pinned and unknown entries stay.
/// Retirement of old installed versions joins it through [`Reclaim`] once its owner exists.
pub(crate) struct CacheReclaim(pub(crate) RuntimeCache);
impl Reclaim for CacheReclaim {
    fn reclaim(&mut self, context: &JobContext) -> io::Result<()> {
        let outcome = self.0.maintain(super::super::cache::utc_seconds()?, context)?;
        tracing::info!(
            operation = "update_space_reclaimed",
            removed = outcome.removed_entries,
            pinned = outcome.pinned_entries,
            "Runtime cache maintenance ran to make room for an update"
        );
        Ok(())
    }
}

/// Require `remaining` bytes plus [`HEADROOM_BYTES`] free at `directory`. On a shortage, run
/// every reclaimer once and measure again; a shortage that remains is reported.
/// # Errors
/// [`SpaceShortage`] (inside an I/O error of kind `StorageFull`) when the space is still not
/// there; cancellation and native query failures unchanged. A failing reclaimer is logged.
pub(crate) fn ensure(
    directory: &Path,
    remaining: u64,
    space: &dyn FreeSpace,
    reclaimers: &mut [&mut dyn Reclaim],
    context: &JobContext,
) -> io::Result<()> {
    check(context)?;
    let needed = remaining.saturating_add(HEADROOM_BYTES);
    let available = space.available(directory)?;
    if remaining == 0 || available >= needed {
        return Ok(());
    }
    for reclaimer in reclaimers.iter_mut() {
        check(context)?;
        // A reclaimer that cannot run now leaves the shortage to be measured and reported.
        if let Err(cause) = reclaimer.reclaim(context) {
            check(context)?;
            tracing::warn!(operation = "update_space_reclaim_failed", cause = %cause,
                "Making room for an update failed; measuring what is free");
        }
    }
    let available = space.available(directory)?;
    if available >= needed {
        return Ok(());
    }
    let shortage = SpaceShortage { needed, available };
    tracing::warn!(
        operation = "update_space_short",
        needed,
        available,
        "Not enough free disk space for the update after reclaiming"
    );
    Err(shortage.into_io())
}

#[cfg(test)]
#[path = "space_tests.rs"]
mod tests;
