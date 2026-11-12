// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resume owned package bytes and retain their exact verified file through extraction.
mod custody;
mod failure;
mod local;
use super::{cache::RuntimeCache, check};
use custody::Custody;
pub use failure::DownloadFailure;
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
}
impl Download {
    /// Open a protected content-addressed entry for a freshly authenticated target.
    /// Run on the execution owner's storage job. Missing payload bytes are a cache miss;
    /// ambiguous ownership is preserved and reported instead of adopting unknown files.
    /// # Errors
    /// Rejects unknown installations, busy writers, unsafe paths, damaged ownership and I/O.
    pub fn open(
        cache: RuntimeCache,
        target: Target,
        context: &JobContext,
    ) -> Result<Self, DownloadFailure> {
        let mut custody = Custody::open(cache, &target, context)?;
        let reset = custody.payload.length()? > target.length();
        if reset {
            custody.payload.reset(context)?;
        }
        Ok(Self { custody, target, reset })
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
        self.custody
            .payload
            .receive_exact(plan.offset(), plan.length(), body, context)
            .map_err(Into::into)
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
