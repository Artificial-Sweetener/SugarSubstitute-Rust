// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose authenticated repository traversal and resumable delivery on admitted jobs.
//!
//! The caller retains this owner across job outcomes and settles HTTP custody before disposal.
//! Each operation uses its supplied finite execution deadline. No application lease, service,
//! scheduling primitive or installation activation is created here. Network calls never hold
//! the trust writer. A failed check is an error, never evidence that the app is current.
mod failure;
use crate::{
    setup_storage::{
        AuthenticatedOffer, Download, OFFER_RESERVATION_BYTES, TrustStore, VerifiedDownload,
    },
    update_http::{self, Http},
};
pub use failure::Failure;
use std::{io, path::Path, time::Duration};
use sugarsubstitute_contracts::{
    execution::JobContext,
    update::{MAX_OFFER_BYTES, Request},
};
use sugarsubstitute_update::{MAX_METADATA_BYTES, http::Policy};

/// Admission for retained discovery bytes plus native trust, transport and download custody.
pub const RESERVATION_BYTES: u64 = crate::setup_storage::TRUST_RESERVATION_BYTES
    + update_http::RESERVATION_BYTES
    + crate::setup_storage::DOWNLOAD_RESERVATION_BYTES
    + OFFER_RESERVATION_BYTES;
const ROOTS_PER_CHECK: usize = 32;

/// A bounded check either authenticates an offer or asks its owner to continue key rotation.
pub enum Check {
    /// Accepted roots are durable; schedule another check before obtaining any offer.
    MoreRoots,
    /// Complete authenticated metadata and the exact locally requested subscription.
    Offer(Box<AuthenticatedOffer>),
}

/// One bounded preparation attempt; interrupted responses return their original error instead.
pub enum Preparation {
    /// A complete HTTP interval was saved; admit another attempt to continue the package.
    Pending {
        /// Durable package prefix in bytes.
        received: u64,
        /// Authenticated full archive length in bytes.
        total: u64,
    },
    /// Every archive byte has been verified and its exact file remains under native custody.
    Complete(Box<VerifiedDownload>),
}

/// Repository transport custody with a distribution-owned origin policy and idle I/O bound.
pub struct Repository {
    http: Http,
    policy: Policy,
    idle: Duration,
}
impl Repository {
    /// Construct an inert owner; endpoint policy is configured locally, never by an offer.
    pub fn new(policy: Policy, idle: Duration) -> Self {
        Self { http: Http::new(), policy, idle }
    }

    /// Whether every native request handle and its admission charge has actually retired.
    pub fn settled(&self) -> bool {
        self.http.settled()
    }

    /// Retry native retirement on an admitted cleanup job, including after cancellation.
    /// # Errors
    /// Retains unresolved custody and the original native retirement failure.
    pub fn retire(&mut self) -> io::Result<()> {
        self.http.retire()
    }

    /// Fetch authenticated references in order and save each floor before the next request.
    /// `now` supplies current Unix UTC seconds after each network operation.
    /// # Errors
    /// Preserves network, signature, clock, ownership and publication causes. Only an exact
    /// clean HTTP 404 for the next root ends rotation; other failures never imply absence.
    pub fn discover(
        &mut self,
        trust: &TrustStore,
        request: Request,
        mut now: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<Check, Failure> {
        self.require_settled()?;
        let memory = context.memory().claim(OFFER_RESERVATION_BYTES).map_err(io::Error::other)?;
        for index in 0..ROOTS_PER_CHECK {
            let version = trust.next_root_version(context)?;
            match self.metadata(&format!("{version}.root.json"), MAX_METADATA_BYTES, context) {
                Ok(root) => trust.rotate(&root, context)?,
                Err(Failure::Http(update_http::Failure::Policy(
                    sugarsubstitute_update::http::Rejection::Status(404),
                ))) => break,
                Err(cause) => return Err(cause),
            }
            if index + 1 == ROOTS_PER_CHECK {
                return Ok(Check::MoreRoots);
            }
        }
        let timestamp = self.metadata("timestamp.json", MAX_METADATA_BYTES, context)?;
        let version = trust.snapshot_version(&timestamp, now(), context)?;
        let snapshot =
            self.metadata(&format!("{version}.snapshot.json"), MAX_METADATA_BYTES, context)?;
        let version = trust.targets_version(&timestamp, &snapshot, now(), context)?;
        let targets =
            self.metadata(&format!("{version}.targets.json"), MAX_METADATA_BYTES, context)?;
        let metadata = sugarsubstitute_update::MetadataSet {
            timestamp: &timestamp,
            snapshot: &snapshot,
            targets: &targets,
        };
        trust.refresh(metadata, now(), context)?;
        let descriptor = self.metadata(&request.target_name(), MAX_OFFER_BYTES, context)?;
        let inputs =
            crate::setup_storage::OfferInputs { request, timestamp, snapshot, targets, descriptor };
        Ok(Check::Offer(Box::new(AuthenticatedOffer::from_parts(
            trust,
            inputs,
            memory,
            now(),
            context,
        )?)))
    }

    /// Reauthorize preparation, open its owned prefix and perform at most one HTTP request.
    /// Complete cached bytes skip HTTP but are fully hashed again. This never activates them.
    /// A deadline/disconnect preserves flushed bytes for a newly admitted retry; no retry loop
    /// hides errors or imposes a global startup timeout. The caller owns retry/backoff policy.
    /// # Errors
    /// Rejects inadmissible/stale offers, changed trust, unsafe storage, framing and byte errors.
    pub fn prepare(
        &mut self,
        discovery: &AuthenticatedOffer,
        trust: &TrustStore,
        installation: &Path,
        mut now: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<Preparation, Failure> {
        self.require_settled()?;
        trust.verify_installation(installation, context)?;
        let authorization = discovery.authorization(trust);
        let offer = trust.offer(
            authorization.metadata,
            authorization.request,
            authorization.descriptor,
            now(),
            context,
        )?;
        let target = offer.into_package()?;
        let name = target.name().to_owned();
        let total = target.length();
        let mut download = Download::open(trust.runtime_cache(context)?, target, context)?;
        if download.offset()? < total {
            self.http.package(
                &self.policy,
                self.policy.asset(&name).map_err(update_http::Failure::from)?,
                &mut download,
                self.idle,
                context,
            )?;
        }
        let received = download.offset()?;
        if received < total {
            return Ok(Preparation::Pending { received, total });
        }
        Ok(Preparation::Complete(Box::new(download.finish(now, context)?)))
    }

    /// Fetch only a bounded flat asset from the configured origin policy.
    fn metadata(
        &mut self,
        name: &str,
        bound: usize,
        context: &JobContext,
    ) -> Result<Vec<u8>, Failure> {
        Ok(self.http.metadata(
            &self.policy,
            self.policy.asset(name).map_err(update_http::Failure::from)?,
            bound,
            self.idle,
            context,
        )?)
    }

    /// An earlier unresolved transport cannot be hidden by a cache-only successful operation.
    fn require_settled(&self) -> Result<(), Failure> {
        if !self.settled() {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Update transport still owns native handles",
            )
            .into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
