// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Feed offline distribution bytes through the same retained cache and signed target verifier.
use super::*;
use crate::setup_storage::{files::Directory, invalid};

/// Source layout is selected locally; untrusted footer bytes never grant target authority.
enum Layout {
    Bare,
    Installer,
}

impl Download {
    /// Import a local bare package for an authenticated target without acquiring application leases.
    /// The source and resolved ancestors remain pinned against writes/deletion throughout copying
    /// and verification. An existing partial cache resumes at its actual length, and the final
    /// digest covers every byte including that prefix. The source is never modified or adopted
    /// as installation-owned content. Execute on an admitted storage worker with
    /// `DOWNLOAD_RESERVATION_BYTES`; `utc_seconds` supplies Unix UTC seconds.
    ///
    /// This returns preparation custody only. Installation publication still requires current
    /// repository authorization, durable rollback checks, and authenticated application health.
    /// # Errors
    /// Rejects unsafe paths, changed size/digest, expired metadata, cancellation, competing cache
    /// ownership and I/O failures. Source rejection before cache open preserves existing bytes.
    /// Later interruption preserves the received prefix; digest failure resets only owned cache.
    pub fn import_file(
        cache: RuntimeCache,
        target: Target,
        source: &Path,
        utc_seconds: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<VerifiedDownload, DownloadFailure> {
        Self::import_local(cache, target, source, Layout::Bare, utc_seconds, context)
    }

    /// Import the signed bare package embedded in an installer executable.
    /// The existing package footer locates the interval before optional PE certificate padding
    /// and the certificate table. Only that interval is copied; the signed target authenticates
    /// all its bytes. Executable code and certificates are neither executed nor granted trust.
    /// The target must already come from the installation's authenticated repository metadata.
    ///
    /// Uses the same source pinning, cache resume, admitted worker reservation and final digest
    /// verification as `import_file`. This permits an installer and online updater to deliver
    /// exactly the same package identity without re-encoding or stripping platform signatures.
    /// # Errors
    /// Preserves malformed envelope, source, cache, cancellation and verification failures.
    /// Invalid bounds or a length mismatch leave the existing cache untouched.
    pub fn import_installer(
        cache: RuntimeCache,
        target: Target,
        source: &Path,
        utc_seconds: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<VerifiedDownload, DownloadFailure> {
        Self::import_local(cache, target, source, Layout::Installer, utc_seconds, context)
    }

    /// Keep source and ancestry custody through the shared cache verification outcome.
    fn import_local(
        cache: RuntimeCache,
        target: Target,
        source: &Path,
        layout: Layout,
        mut utc_seconds: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<VerifiedDownload, DownloadFailure> {
        check(context)?;
        crate::setup_storage::files::lexical(source)?;
        let parent = Directory::open(
            source.parent().ok_or_else(|| invalid("Local package parent is absent"))?,
            false,
        )?;
        let leaf = source
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| invalid("Local package filename is invalid"))?;
        let mut source = parent.read(leaf)?;
        let range = match layout {
            Layout::Bare => 0..source.metadata()?.len(),
            Layout::Installer => crate::setup_storage::packed::embedded_range(&mut source)?,
        };
        let length = target.length();
        if range.end - range.start != length {
            return Err(Rejection::BodyLength.into());
        }
        target.require_valid_at(utc_seconds())?;
        check(context)?;
        let mut download = Self::open(cache, target, context)?;
        let offset = download.offset()?;
        source.seek(SeekFrom::Start(range.start + offset))?;
        let mut remaining = (&mut source).take(length - offset);
        download.receive_interval(offset, length - offset, &mut remaining, context)?;
        download.finish(utc_seconds, context)
    }
}
