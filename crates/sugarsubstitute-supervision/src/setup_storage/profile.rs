// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Grant private installer profiles before SDK effects and finalize only after actual browser release.
use super::cache;
use super::cache::CacheLease;
use super::cache::Namespace;
use super::cache::RuntimeCache;
use super::cache_custody::CacheFamilyId;
use super::check;
use super::invalid;
use crate::durable_storage::files::Directory;
use sha2::{Digest, Sha256};
use std::{
    io,
    path::{Path, PathBuf},
};
use sugarsubstitute_contracts::execution::JobContext;
const PREFIX: &str = "setup-browser-v1";
const PROFILE_BYTES: u64 = 128 * 1024 * 1024;

/// Original cache custody through environment construction, controller use and browser retirement.
/// Only private installer content belongs here; application browser sessions remain authoritative.
pub struct InstallerProfile {
    lease: CacheLease,
    recovering: bool,
    verified: bool,
}
impl InstallerProfile {
    /// Prepare one unfinished private profile for inert recovery, or a fresh bounded SDK grant.
    /// Run on an 8 MiB storage job alongside the cache scope's original metadata admission.
    /// `attempted` contains bounded exact generation names already attempted during this startup.
    /// # Errors
    /// Preserves unknown ownership, pressure, current cancellation and actual native I/O failures.
    pub fn prepare(
        cache: &RuntimeCache,
        attempted: &[String],
        context: &JobContext,
    ) -> io::Result<Self> {
        check(context)?;
        if attempted.len() > 8 {
            return Err(invalid("Browser recovery attempt bound exceeded"));
        }
        let mut pending = cache.pending(Namespace::SetupBrowser, context)?;
        while attempted.len() < 8 {
            let Some(lease) = pending.next(context)? else {
                break;
            };
            if attempted.iter().any(|key| generation(&lease).is_ok_and(|actual| &actual == key)) {
                continue;
            }
            if lease.producer() != Some(descriptor(&lease)?.as_str()) {
                tracing::warn!(
                    operation = "setup_browser_recovery_miss",
                    "Preserving an unrecognized browser producer"
                );
                continue;
            }
            return Ok(Self { lease, recovering: true, verified: false });
        }
        drop(pending);
        let unique = CacheFamilyId::new(context)?;
        let mut lease = cache.clone().fresh(
            Namespace::SetupBrowser,
            &cache::storage_key(format!("{PREFIX}:{}", unique.encoded()).as_bytes()),
            PROFILE_BYTES,
            cache::utc_seconds()?,
            context,
        )?;
        lease.begin_generated(&descriptor(&lease)?, context)?;
        Ok(Self { lease, recovering: false, verified: false })
    }
    /// Pinned opaque SDK path selected by native storage, independently of presentation state.
    pub fn path(&self) -> PathBuf {
        self.lease.path()
    }
    /// Stable physical generation name used solely to bound startup recovery attempts.
    /// # Errors
    /// Rejects a malformed internal generation path without selecting any fallback directory.
    pub fn generation(&self) -> io::Result<String> {
        generation(&self.lease)
    }
    /// Whether this profile must run an inert recovery controller before a fresh UI is admitted.
    pub fn recovering(&self) -> bool {
        self.recovering
    }
    /// Verify WebView2's actual native folder before controller authorization, off the UI thread.
    /// The cache lease already retains the expected directory ancestry and cross-process exclusion.
    /// # Errors
    /// Rejects duplicate verification, redirection, changed identity, links and cancellation.
    pub fn verify(&mut self, observed: &Path, context: &JobContext) -> io::Result<()> {
        check(context)?;
        if self.verified {
            return Err(invalid("Browser profile already verified"));
        }
        let expected = Directory::open(&self.lease.path(), false)?;
        let observed = Directory::open(observed, false)?;
        if expected.identity()? != observed.identity()? {
            return Err(invalid("Installer browser profile was redirected"));
        }
        self.verified = true;
        Ok(())
    }
    /// Whether this attempt retains the exact native identity authorized for controller use.
    pub fn verified(&self) -> bool {
        self.verified
    }
    /// Inventory and journal retirement only after the trusted UI adapter proves collection exit.
    /// `released` also permits a fresh grant never exposed to any SDK creation call.
    /// False preserves its durable unfinished producer for a later inert recovery attempt.
    /// Keep this owner on failure and retry with a current 8 MiB storage job.
    /// # Errors
    /// Preserves damaged output, cancellation, bounded inventory and actual native storage failures.
    pub fn finish(&mut self, released: bool, context: &JobContext) -> io::Result<()> {
        check(context)?;
        if !released {
            return Ok(());
        }
        if self.lease.producer().is_some() {
            match self.lease.record_generated(cache::utc_seconds()?, context) {
                Ok(_) => {}
                Err(cause) if cause.kind() == io::ErrorKind::StorageFull => {
                    tracing::warn!(operation="setup_browser_cache_overflow", cause=%cause,
                        "Released private browser output exceeded its grant");
                }
                Err(cause) => return Err(cause),
            }
        }
        self.lease.retire(context)
    }
}
/// Bind restart matching facts to the actual private SDK directory without ambient paths.
fn descriptor(lease: &CacheLease) -> io::Result<String> {
    let identity = Directory::open(&lease.path(), false)?.identity()?;
    let mut digest = Sha256::new();
    digest.update(PREFIX.as_bytes());
    for word in [identity.0, identity.1, identity.2] {
        digest.update(word.to_le_bytes());
    }
    Ok(format!("{PREFIX}:{:x}", digest.finalize()))
}
/// Storage already restricts this component to its fixed canonical generation syntax.
fn generation(lease: &CacheLease) -> io::Result<String> {
    lease
        .path()
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .ok_or_else(|| invalid("Browser cache generation is absent"))
}

#[cfg(test)]
mod tests;
