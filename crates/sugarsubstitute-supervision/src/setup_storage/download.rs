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
    io::{self, Read, Seek, SeekFrom, Write},
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
        let custody = Custody::open(cache, &target, context)?;
        let reset = custody.file.metadata()?.len() > target.length();
        if reset {
            custody.file.set_len(0)?;
            custody.file.sync_all()?;
        }
        Ok(Self { custody, target, reset })
    }
    /// Actual retained prefix length; transport should request this byte offset.
    /// # Errors
    /// Returns native metadata failures without guessing an offset.
    pub fn offset(&self) -> Result<u64, DownloadFailure> {
        Ok(self.custody.file.metadata()?.len())
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
        self.receive_interval(plan.offset(), plan.length(), body, context)
    }

    /// Flush partial bytes and retain both causes when copying and durable storage fail.
    fn receive_interval(
        &mut self,
        offset: u64,
        length: u64,
        body: &mut impl Read,
        context: &JobContext,
    ) -> Result<(), DownloadFailure> {
        let outcome = self.copy(offset, length, body, context);
        match (outcome, self.custody.file.sync_all()) {
            (result, Ok(())) => result,
            (Ok(()), Err(flush)) => Err(flush.into()),
            (Err(transfer), Err(flush)) => {
                Err(DownloadFailure::Flush { transfer: Box::new(transfer), flush })
            }
        }
    }

    /// Copy only the permitted interval, including an extra-byte probe before success.
    fn copy(
        &mut self,
        offset: u64,
        length: u64,
        body: &mut impl Read,
        context: &JobContext,
    ) -> Result<(), DownloadFailure> {
        if offset.checked_add(length).is_none_or(|end| end > self.target.length()) {
            return Err(Rejection::Offset.into());
        }
        if offset == 0 {
            self.custody.file.set_len(0)?;
        }
        self.custody.file.seek(SeekFrom::Start(offset))?;
        let mut scratch = vec![0; CHUNK];
        let mut copied = 0;
        loop {
            check(context)?;
            let maximum = (length - copied).saturating_add(1).min(CHUNK as u64) as usize;
            let count = body.read(&mut scratch[..maximum])?;
            check(context)?;
            if count == 0 {
                break;
            }
            if count > maximum || count as u64 > length - copied {
                return Err(Rejection::BodyLength.into());
            }
            self.custody.file.write_all(&scratch[..count])?;
            copied += count as u64;
        }
        if copied != length {
            return Err(Rejection::BodyLength.into());
        }
        Ok(())
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
        self.custody.file.sync_all()?;
        let mut verifier = self.target.verifier(utc_seconds())?;
        self.custody.file.seek(SeekFrom::Start(0))?;
        let mut scratch = vec![0; CHUNK];
        loop {
            check(context)?;
            let count = self.custody.file.read(&mut scratch)?;
            if count == 0 {
                break;
            }
            verifier.push(&scratch[..count])?;
        }
        check(context)?;
        let verified = match verifier.finish(utc_seconds()) {
            Ok(verified) => verified,
            Err(cause @ sugarsubstitute_update::Error::Digest) => {
                if let Err(flush) =
                    self.custody.file.set_len(0).and_then(|()| self.custody.file.sync_all())
                {
                    return Err(DownloadFailure::Flush { transfer: Box::new(cause.into()), flush });
                }
                return Err(cause.into());
            }
            Err(cause) => return Err(cause.into()),
        };
        self.custody.file.seek(SeekFrom::Start(0))?;
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
        self.custody.file.read(output)
    }
}
impl Seek for VerifiedDownload {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.custody.file.seek(position)
    }
}

#[cfg(test)]
mod tests;
