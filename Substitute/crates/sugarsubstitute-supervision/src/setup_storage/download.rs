// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resume owned package bytes and retain their exact verified file through extraction.
mod custody;
mod failure;
mod local;
pub(super) mod space;
use super::{cache::RuntimeCache, check};
use custody::Custody;
pub use failure::DownloadFailure;
use space::{CacheReclaim, FreeSpace, HEADROOM_BYTES, Reclaim, SpaceShortage, VolumeSpace};
use std::{
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_update::{
    Target, VerifiedBytes,
    delivery::{Rejection, Response, Transfer},
};

/// Admission for one retained cache entry, fixed streaming buffer and protected native opens.
pub const DOWNLOAD_RESERVATION_BYTES: u64 = 16 * 1024 * 1024;
const CHUNK: usize = 256 * 1024;

/// Exclusive package preparation; it never acquires the application's lease or starts work.
pub struct Download {
    custody: Custody,
    target: Target,
    reset: bool,
    space: Box<dyn FreeSpace + Send + Sync>,
}
impl Download {
    /// Open a protected content-addressed entry for a freshly authenticated target.
    /// Run on the execution owner's storage job. Missing payload bytes are a cache miss;
    /// ambiguous ownership is preserved and reported instead of adopting unknown files.
    /// The volume must have the bytes still to download plus [`space::HEADROOM_BYTES`] free;
    /// on a shortage runtime cache maintenance runs first.
    /// # Errors
    /// Rejects unknown installations, busy writers, unsafe paths, damaged ownership and I/O;
    /// [`DownloadFailure::InsufficientSpace`] when the disk is short after reclaiming, and
    /// [`DownloadFailure::CacheBudget`] when the cache's own budget refuses the package.
    pub fn open(
        cache: RuntimeCache,
        target: Target,
        context: &JobContext,
    ) -> Result<Self, DownloadFailure> {
        Self::open_with(cache, target, Box::new(VolumeSpace), &mut [], context)
    }

    /// [`Self::open`] with the free-space measure and reclaimers beyond cache maintenance
    /// supplied by the caller, such as retirement of old installed versions.
    /// # Errors
    /// As [`Self::open`].
    pub fn open_with(
        cache: RuntimeCache,
        target: Target,
        space: Box<dyn FreeSpace + Send + Sync>,
        reclaimers: &mut [&mut dyn Reclaim],
        context: &JobContext,
    ) -> Result<Self, DownloadFailure> {
        let mut maintenance = CacheReclaim(cache.clone());
        let mut custody = Custody::open(cache, &target, context)?;
        let reset = custody.payload.length()? > target.length();
        if reset {
            custody.payload.reset(context)?;
        }
        let remaining = target.length().saturating_sub(custody.payload.length()?);
        let mut chain = Chain(&mut maintenance, reclaimers);
        space::ensure(custody.directory(), remaining, space.as_ref(), &mut [&mut chain], context)?;
        Ok(Self { custody, target, reset, space })
    }
    /// Actual retained prefix length; transport should request this byte offset.
    /// # Errors
    /// Returns native metadata failures without guessing an offset.
    pub fn offset(&self) -> Result<u64, DownloadFailure> {
        Ok(self.custody.payload.length()?)
    }
    /// An oversized owned cache payload was discarded as an observable miss at open.
    pub fn cache_was_reset(&self) -> bool {
        self.reset
    }

    /// Consume one already-open response from an admitted, cancellable transport boundary.
    /// `body.read` must itself have bounded I/O; this owner creates no threads or network calls.
    /// Interrupted transfers flush their received prefix. A server returning 200 restarts it.
    /// # Errors
    /// Rejects framing before mutation, overlong/truncated streams, cancellation and disk errors.
    pub fn receive(
        &mut self,
        response: Response<'_>,
        body: &mut impl Read,
        context: &JobContext,
    ) -> Result<(), DownloadFailure> {
        check(context)?;
        let plan = Transfer::accept(&self.target, self.offset()?, response)?;
        let received =
            self.custody.payload.receive_exact(plan.offset(), plan.length(), body, context);
        received.map_err(|failure| self.classify(failure.into()))
    }

    /// A write the disk refused is a shortage of the bytes still to download; the received
    /// prefix was flushed and stays for a later attempt. The cache's own budget and every
    /// other failure keep their meaning.
    fn classify(&self, failure: DownloadFailure) -> DownloadFailure {
        match failure {
            DownloadFailure::Storage(cause) if cause.kind() == io::ErrorKind::StorageFull => {
                match self.shortage() {
                    Some(shortage) => DownloadFailure::InsufficientSpace(shortage),
                    None => DownloadFailure::Storage(cause),
                }
            }
            DownloadFailure::Flush { transfer, flush } => {
                DownloadFailure::Flush { transfer: Box::new(self.classify(*transfer)), flush }
            }
            other => other,
        }
    }

    /// The shortage now: what is still to download plus headroom, against what is free.
    fn shortage(&self) -> Option<SpaceShortage> {
        let remaining = self.target.length().saturating_sub(self.offset().ok()?);
        let available = self.space.available(self.custody.directory()).ok()?;
        Some(SpaceShortage { needed: remaining.saturating_add(HEADROOM_BYTES), available })
    }

    /// Verify every byte, including a resumed prefix, and retain the same exclusive file handle.
    /// Corrupt complete cache content resets only this declared payload for a later fresh retry.
    /// Expired metadata leaves bytes reusable after a new authenticated metadata refresh.
    /// The bounded clock callback supplies Unix UTC seconds before and after hashing.
    /// # Errors
    /// Rejects truncation, digest mismatch, expired authority, cancellation and native I/O errors.
    pub fn finish(
        mut self,
        mut utc_seconds: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<VerifiedDownload, DownloadFailure> {
        check(context)?;
        if self.offset()? != self.target.length() {
            return Err(Rejection::BodyLength.into());
        }
        self.custody.payload.flush()?;
        let mut verifier = self.target.verifier(utc_seconds())?;
        self.custody.payload.seek(SeekFrom::Start(0))?;
        let _scratch = context.memory().claim(CHUNK as u64).map_err(io::Error::other)?;
        let mut scratch = Vec::new();
        scratch.try_reserve_exact(CHUNK).map_err(io::Error::other)?;
        if scratch.capacity() > CHUNK {
            return Err(super::invalid("Package hash buffer exceeds its admission").into());
        }
        scratch.resize(CHUNK, 0);
        loop {
            check(context)?;
            let count = self.custody.payload.read(&mut scratch)?;
            if count == 0 {
                break;
            }
            verifier.push(&scratch[..count])?;
        }
        check(context)?;
        let verified = match verifier.finish(utc_seconds()) {
            Ok(verified) => verified,
            Err(cause @ sugarsubstitute_update::Error::Digest) => {
                if let Err(flush) = self.custody.payload.reset(context) {
                    return Err(DownloadFailure::Flush { transfer: Box::new(cause.into()), flush });
                }
                return Err(cause.into());
            }
            Err(cause) => return Err(cause.into()),
        };
        self.custody.payload.seek(SeekFrom::Start(0))?;
        Ok(VerifiedDownload { custody: self.custody, verified })
    }
}

/// Cache maintenance first, then the caller's reclaimers; each one's failure is logged and the
/// rest still run, so the shortage that remains is what gets measured.
struct Chain<'a, 'b>(&'a mut CacheReclaim, &'a mut [&'b mut dyn Reclaim]);
impl Reclaim for Chain<'_, '_> {
    fn reclaim(&mut self, context: &JobContext) -> io::Result<()> {
        let first = self.0.reclaim(context);
        for reclaimer in
            std::iter::once(first).chain(self.1.iter_mut().map(|next| next.reclaim(context)))
        {
            if let Err(cause) = reclaimer {
                check(context)?;
                tracing::warn!(operation = "update_space_reclaim_failed", cause = %cause,
                    "Making room for an update failed; the next reclaimer still runs");
            }
        }
        Ok(())
    }
}

/// Read/seek access to the exact verified file; no path reopening or writable handle is exposed.
/// Authentication is for preparation. Activation must revalidate current repository authority.
pub struct VerifiedDownload {
    custody: Custody,
    verified: VerifiedBytes,
}
impl VerifiedDownload {
    /// Signed identity and size of the retained bytes.
    pub fn identity(&self) -> &VerifiedBytes {
        &self.verified
    }
}
impl Read for VerifiedDownload {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.custody.payload.read(output)
    }
}
impl Seek for VerifiedDownload {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.custody.payload.seek(position)
    }
}

#[cfg(test)]
mod tests;
