// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reauthorize delivered or protected published releases under current durable trust.
use super::*;
use sugarsubstitute_contracts::update::{Component, Platform};
use sugarsubstitute_update::{
    ENGINE_PROTOCOL, ENGINE_RELEASE, Error as Verification, ReleaseHistory, VerifiedBytes,
    VerifiedOffer, package,
};

/// Untrusted verification inputs and the installation's retained trust owner.
/// This is a request, not a reusable authorization token. Publication rechecks it under exclusion.
pub struct UpdateAuthorization<'a> {
    /// Trust opened for this exact native installation and its configured repository.
    pub trust: &'a TrustStore,
    /// Complete signed metadata obtained by discovery; no network work occurs during publication.
    pub metadata: MetadataSet<'a>,
    /// Locally owned subscription and release-history facts, never populated from the offer.
    pub request: &'a Request,
    /// Exact descriptor bytes authenticated as this subscription's signed target.
    pub descriptor: &'a [u8],
}

mod staged;
use staged::{Authority, Held};

/// Exact downloaded bytes or a protected publication whose application is independently pinned.
#[derive(Clone, Copy)]
enum Evidence<'a> {
    Archive(&'a VerifiedBytes),
    Published(&'a PublishedRelease),
}
impl UpdateAuthorization<'_> {
    /// Check delivered bytes without advancing release floors or granting publication rights.
    /// Extraction can outlive metadata validity, so its owner calls this on both sides of work.
    /// Durable trust is reloaded under its own writer; no network or extraction holds that writer.
    pub(in crate::setup_storage) fn verify_delivery(
        &self,
        installation: &Path,
        bytes: &VerifiedBytes,
        component: Component,
        now: i64,
        context: &JobContext,
    ) -> Result<(), TrustFailure> {
        self.trust.verify_installation(installation, context)?;
        Self::require_native_request(self.request, component)?;
        self.trust.protected(context, |protection| {
            let _writer = self.trust.writer(protection, context)?;
            self.trust.require_subscription(self.request, protection, context)?;
            let history = self.trust.read_releases(protection)?;
            let mut trust = self.trust.read(protection)?;
            self.trust.apply(&mut trust, protection, context, |trust| {
                trust.refresh(
                    MetadataSet {
                        timestamp: self.metadata.timestamp,
                        snapshot: self.metadata.snapshot,
                        targets: self.metadata.targets,
                    },
                    now,
                )?;
                self.verify(trust, &history, Evidence::Archive(bytes), now).map(|_| ())
            })
        })
    }

    /// Require locally supplied facts to describe this native application engine before delivery.
    pub(in crate::setup_storage) fn require_application_request(
        request: &Request,
    ) -> io::Result<()> {
        Self::require_native_request(request, Component::Application)
    }

    /// The receiving component supplies its role; all engine capabilities remain local facts.
    pub(in crate::setup_storage) fn require_native_request(
        request: &Request,
        component: Component,
    ) -> io::Result<()> {
        let expected = Self::local_request(request.channel)?;
        if request.component != component
            || request.platform != expected.platform
            || request.updater_release != expected.updater_release
            || request.updater_protocol != expected.updater_protocol
            || request.package_schema != expected.package_schema
        {
            return Err(invalid("Update request does not describe this application engine"));
        }
        Ok(())
    }

    /// Actual native engine facts; healthy release identity is supplied only by installed state.
    pub(in crate::setup_storage) fn local_request(
        channel: sugarsubstitute_contracts::update::Channel,
    ) -> io::Result<Request> {
        let platform = match std::env::consts::ARCH {
            "x86_64" => Platform::WindowsX86_64,
            "aarch64" => Platform::WindowsAarch64,
            _ => return Err(invalid("No native update package is supported for this host")),
        };
        Ok(Request {
            channel,
            platform,
            component: Component::Application,
            sequence_floor: 0,
            sequence_digest: None,
            updater_release: ENGINE_RELEASE,
            updater_protocol: ENGINE_PROTOCOL,
            package_schema: package::SCHEMA_VERSION,
            intent: sugarsubstitute_contracts::installation::PreparationIntent::Automatic,
        })
    }

    /// Retain trust exclusion across local publication so rotation cannot race the final check.
    pub(in crate::setup_storage) fn publish<T>(
        &self,
        installation: &Directory,
        bytes: &VerifiedBytes,
        now: impl FnMut() -> i64,
        context: &JobContext,
        publish: impl FnOnce(&VerifiedOffer, &mut dyn FnMut() -> io::Result<()>) -> io::Result<T>,
    ) -> Result<T, TrustFailure> {
        Self::require_application_request(self.request)?;
        self.publish_evidence(installation, Evidence::Archive(bytes), now, context, publish)
    }

    /// Publish only updater state, under the same signature, history, clock and trust-writer rules.
    pub(in crate::setup_storage) fn publish_updater<T>(
        &self,
        bytes: &VerifiedBytes,
        now: impl FnMut() -> i64,
        context: &JobContext,
        publish: impl FnOnce(&VerifiedOffer, &mut dyn FnMut() -> io::Result<()>) -> io::Result<T>,
    ) -> Result<T, TrustFailure> {
        Self::require_native_request(self.request, Component::Updater)?;
        self.publish_evidence(
            &self.trust._installation_root,
            Evidence::Archive(bytes),
            now,
            context,
            publish,
        )
    }

    /// Reauthorize already published application bytes without reopening a provisioning archive.
    /// The caller must retain verified application payloads alongside this protected receipt.
    pub(in crate::setup_storage) fn publish_prepared<T>(
        &self,
        installation: &Directory,
        publication: &PublishedRelease,
        now: impl FnMut() -> i64,
        context: &JobContext,
        publish: impl FnOnce(&VerifiedOffer, &mut dyn FnMut() -> io::Result<()>) -> io::Result<T>,
    ) -> Result<T, TrustFailure> {
        publication.require_installation(installation)?;
        Self::require_application_request(self.request)?;
        self.publish_evidence(installation, Evidence::Published(publication), now, context, publish)
    }

    /// Both delivery forms share the same trust writer, clock checks and publication floors.
    /// For short local publications only; long staging uses [`Self::authenticate`] and
    /// [`staged::StagedDelivery::commit`] so no payload work runs under the trust writer.
    fn publish_evidence<T>(
        &self,
        installation: &Directory,
        evidence: Evidence<'_>,
        mut now: impl FnMut() -> i64,
        context: &JobContext,
        publish: impl FnOnce(&VerifiedOffer, &mut dyn FnMut() -> io::Result<()>) -> io::Result<T>,
    ) -> Result<T, TrustFailure> {
        self.require_installation(installation)?;
        self.trust.protected(context, |protection| {
            let _writer = self.trust.writer(protection, context)?;
            self.verify_held(evidence, None, &mut now, protection, context)?
                .publish(self, evidence, &mut now, protection, context, publish)
        })
    }

    /// Trust and its signed inputs must belong to this exact native installation and engine.
    fn require_installation(&self, installation: &Directory) -> Result<(), TrustFailure> {
        if installation.identity()? != self.trust._installation_root.identity()? {
            return Err(invalid("Update trust belongs to another installation").into());
        }
        Self::require_native_request(self.request, self.request.component)?;
        Ok(())
    }

    /// Reread trust, subscription and release history under the caller's held writer, then
    /// refresh and verify the evidence. `expected` is the authority a staged delivery observed;
    /// any change fails with a typed error before verification can report a weaker cause.
    fn verify_held(
        &self,
        evidence: Evidence<'_>,
        expected: Option<Authority>,
        now: &mut dyn FnMut() -> i64,
        protection: &mut Protection,
        context: &JobContext,
    ) -> Result<Held, TrustFailure> {
        let mut trust = self.trust.read(protection)?;
        let subscription = if self.request.intent
            == sugarsubstitute_contracts::installation::PreparationIntent::Automatic
        {
            Some(self.trust.subscription_locked(protection, context)?)
        } else {
            None
        };
        let authority = Authority { root_version: trust.root_version()?, subscription };
        if let Some(expected) = expected {
            expected.require(authority)?;
        }
        if subscription.is_some_and(|value| value.channel() != self.request.channel) {
            return Err(invalid("Update subscription changed during automatic preparation").into());
        }
        let history = self.trust.read_releases(protection)?;
        let mut time = i64::MIN;
        let offer = self.trust.apply(&mut trust, protection, context, |trust| {
            time = now();
            trust.refresh(
                MetadataSet {
                    timestamp: self.metadata.timestamp,
                    snapshot: self.metadata.snapshot,
                    targets: self.metadata.targets,
                },
                time,
            )?;
            self.verify(trust, &history, evidence, time)
        })?;
        Ok(Held { trust, history, offer, authority, time })
    }

    /// Verify again inside the held writer; the injected clock may not move backwards.
    fn recheck(
        &self,
        trust: &RepositoryTrust,
        history: &ReleaseHistory,
        evidence: Evidence<'_>,
        last_time: &mut i64,
        now: &mut dyn FnMut() -> i64,
        context: &JobContext,
    ) -> io::Result<()> {
        check(context)?;
        let time = now();
        if time < *last_time {
            return Err(io::Error::other(Verification::ClockRollback));
        }
        self.verify(trust, history, evidence, time).map_err(io::Error::other)?;
        *last_time = time;
        check(context)
    }
    /// Match the current signed target to retained delivery or a protected publication receipt.
    fn verify(
        &self,
        trust: &RepositoryTrust,
        history: &ReleaseHistory,
        evidence: Evidence<'_>,
        now: i64,
    ) -> Result<VerifiedOffer, Verification> {
        let offer = trust.offer(self.request, self.descriptor, now)?;
        history.check(self.request, &offer)?;
        if offer.admission() != sugarsubstitute_contracts::update::Admission::Prepare {
            return Err(Verification::Admission);
        }
        let matches = match evidence {
            Evidence::Archive(bytes) => {
                offer.offer().package_name() == bytes.name()
                    && offer.offer().package_bytes() == bytes.length()
                    && offer.offer().package_digest() == bytes.digest()
            }
            Evidence::Published(publication) => publication.matches(self.request, &offer),
        };
        if !matches {
            return Err(Verification::Digest);
        }
        Ok(offer)
    }
}
