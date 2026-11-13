// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep long payload staging outside the trust writer while the final check stays atomic.
//!
//! Preparation runs in three steps. [`UpdateAuthorization::authenticate`] verifies the offer and
//! runs a short reservation under one writer hold. The caller then stages and verifies payload
//! bytes holding only its preparation and setup exclusions, so trust readers and other short
//! trust operations proceed meanwhile. [`StagedDelivery::commit`] takes the writer again, rereads
//! trust and release history, verifies the same evidence against them, authorizes the release
//! history and runs the caller's publication in that one hold, handing it the receipt facts
//! ([`PreparedRelease`]) for exactly what that hold verified. A root rotation, subscription
//! change or different authorized package in between fails the commit with
//! [`TrustFailure::AuthorityChanged`] before anything is published.
use super::*;
use crate::setup_storage::AuthorityChange;
use sugarsubstitute_contracts::update::subscription::Subscription;
use sugarsubstitute_update::PreparedRelease;

/// Local authority observed under the trust writer and required unchanged at commit.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct Authority {
    pub(super) root_version: u32,
    /// Present for automatic preparation, which follows the local subscription.
    pub(super) subscription: Option<Subscription>,
}
impl Authority {
    /// Report the most fundamental change first; equal authority passes.
    pub(super) fn require(self, current: Self) -> Result<(), TrustFailure> {
        if self.root_version != current.root_version {
            return Err(TrustFailure::AuthorityChanged(AuthorityChange::RootRotated));
        }
        if self.subscription != current.subscription {
            return Err(TrustFailure::AuthorityChanged(AuthorityChange::SubscriptionChanged));
        }
        Ok(())
    }
}

/// Trust state reread and verified under a writer the caller still holds.
pub(super) struct Held {
    pub(super) trust: RepositoryTrust,
    pub(super) history: ReleaseHistory,
    pub(super) offer: VerifiedOffer,
    pub(super) authority: Authority,
    pub(super) time: i64,
}
impl Held {
    /// Authorize the release history, then publish with rechecks inside the same writer hold.
    pub(super) fn publish<T>(
        self,
        authorization: &UpdateAuthorization<'_>,
        evidence: Evidence<'_>,
        now: &mut dyn FnMut() -> i64,
        protection: &Protection,
        context: &JobContext,
        publish: impl FnOnce(&VerifiedOffer, &mut dyn FnMut() -> io::Result<()>) -> io::Result<T>,
    ) -> Result<T, TrustFailure> {
        let Self { trust, history, offer, mut time, .. } = self;
        let next = authorization.authorize_history(&history, &offer, protection, context)?;
        let mut recheck =
            || authorization.recheck(&trust, &next, evidence, &mut time, &mut *now, context);
        recheck()?;
        publish(&offer, &mut recheck).map_err(TrustFailure::Storage)
    }
}

/// An offer authenticated under a writer hold that has since been released.
/// It grants no publication right: [`Self::commit`] verifies current authority again.
pub(in crate::setup_storage) struct StagedDelivery<'a> {
    authorization: &'a UpdateAuthorization<'a>,
    evidence: Evidence<'a>,
    offer: VerifiedOffer,
    authority: Authority,
}
impl UpdateAuthorization<'_> {
    /// Durably advance the release floor to `offer` under the caller's held writer.
    fn authorize_history(
        &self,
        history: &ReleaseHistory,
        offer: &VerifiedOffer,
        protection: &Protection,
        context: &JobContext,
    ) -> Result<ReleaseHistory, TrustFailure> {
        let next = history.authorize(self.request, offer)?;
        self.trust.write_releases(history, &next, protection, context)?;
        Ok(next)
    }
}
impl<'a> UpdateAuthorization<'a> {
    /// Authenticate delivered application bytes and run `reserve` under one short writer hold.
    /// `reserve` may persist resumable intent; it must not stage payloads or publish anything
    /// launchable. The release floor advances here so reserved intent never precedes it, and the
    /// commit authorizes it again against the history current at that point.
    /// # Errors
    /// Rejects foreign installations, changed subscriptions, invalid or expired metadata,
    /// rollback, mismatched bytes, lock timeout, cancellation and `reserve` failures.
    pub(in crate::setup_storage) fn authenticate<R>(
        &'a self,
        installation: &Directory,
        bytes: &'a VerifiedBytes,
        now: impl FnMut() -> i64,
        context: &JobContext,
        reserve: impl FnOnce(&VerifiedOffer, &mut dyn FnMut() -> io::Result<()>) -> io::Result<R>,
    ) -> Result<(StagedDelivery<'a>, R), TrustFailure> {
        Self::require_application_request(self.request)?;
        self.authenticate_archive(installation, bytes, now, context, reserve)
    }

    /// Authenticate delivered updater-engine bytes as [`Self::authenticate`] does.
    /// # Errors
    /// As [`Self::authenticate`], and rejects requests that do not describe the updater.
    pub(in crate::setup_storage) fn authenticate_updater<R>(
        &'a self,
        bytes: &'a VerifiedBytes,
        now: impl FnMut() -> i64,
        context: &JobContext,
        reserve: impl FnOnce(&VerifiedOffer, &mut dyn FnMut() -> io::Result<()>) -> io::Result<R>,
    ) -> Result<(StagedDelivery<'a>, R), TrustFailure> {
        Self::require_native_request(self.request, Component::Updater)?;
        self.authenticate_archive(&self.trust._installation_root, bytes, now, context, reserve)
    }

    /// Shared first hold once the caller has checked the request's component.
    fn authenticate_archive<R>(
        &'a self,
        installation: &Directory,
        bytes: &'a VerifiedBytes,
        mut now: impl FnMut() -> i64,
        context: &JobContext,
        reserve: impl FnOnce(&VerifiedOffer, &mut dyn FnMut() -> io::Result<()>) -> io::Result<R>,
    ) -> Result<(StagedDelivery<'a>, R), TrustFailure> {
        self.require_installation(installation)?;
        let evidence = Evidence::Archive(bytes);
        self.trust.protected(context, |protection| {
            let _writer = self.trust.writer(protection, context)?;
            let Held { trust, history, offer, authority, mut time } =
                self.verify_held(evidence, None, &mut now, protection, context)?;
            // Reserved intent may name this release, so its floor is durable first.
            let history = self.authorize_history(&history, &offer, protection, context)?;
            let reserved = reserve(&offer, &mut || {
                self.recheck(&trust, &history, evidence, &mut time, &mut now, context)
            })?;
            Ok((StagedDelivery { authorization: self, evidence, offer, authority }, reserved))
        })
    }
}
impl StagedDelivery<'_> {
    /// Reverify against current trust and release history, authorize the history and run
    /// `publish` in one writer hold. `publish` receives the verified offer, the receipt facts
    /// that this hold's trust binds to it, and a recheck for use between its writes.
    /// # Errors
    /// Returns `AuthorityChanged` when the root rotated, the subscription changed or current
    /// trust authorizes a different package; verification, storage, lock and cancellation
    /// failures keep their causes. Nothing is published on any failure before `publish` runs.
    pub(in crate::setup_storage) fn commit<T>(
        self,
        mut now: impl FnMut() -> i64,
        context: &JobContext,
        publish: impl FnOnce(
            &VerifiedOffer,
            &PreparedRelease,
            &mut dyn FnMut() -> io::Result<()>,
        ) -> io::Result<T>,
    ) -> Result<T, TrustFailure> {
        let authorization = self.authorization;
        authorization.trust.protected(context, |protection| {
            let _writer = authorization.trust.writer(protection, context)?;
            let held = authorization.verify_held(
                self.evidence,
                Some(self.authority),
                &mut now,
                protection,
                context,
            )?;
            let (before, after) = (self.offer.offer(), held.offer.offer());
            if before.sequence() != after.sequence()
                || before.package_digest() != after.package_digest()
                || before.package_name() != after.package_name()
            {
                return Err(TrustFailure::AuthorityChanged(AuthorityChange::OfferChanged));
            }
            let release = held.trust.prepared_release(
                authorization.request,
                &held.offer,
                authorization.descriptor,
                held.time,
            )?;
            held.publish(
                authorization,
                self.evidence,
                &mut now,
                protection,
                context,
                |offer, recheck| publish(offer, &release, recheck),
            )
        })
    }
}
