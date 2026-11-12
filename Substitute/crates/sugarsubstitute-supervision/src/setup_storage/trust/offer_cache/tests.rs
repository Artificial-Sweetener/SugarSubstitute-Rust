// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native cache publication, hostile inputs and current-authority verification after reopen.
use super::*;
use crate::setup_storage::{test_package::repository::SignedPackage, test_worker as worker};
use sugarsubstitute_contracts::update::{Admission, Channel};
use sugarsubstitute_update::fixture::{EXPIRES, NOW, key, root};
type TestResult = worker::TestResult;

#[test]
fn reopened_cache_uses_current_local_facts_and_current_signing_authority() -> TestResult {
    worker::run_reserved(crate::setup_storage::SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let installation = worker::installation(92, context)?;
        let signed = SignedPackage::new(b"package", 1)?;
        let trust = TrustStore::initialize(installation.path(), [92; 32], &signed.root, context)?;
        let offer = AuthenticatedOffer::authenticate(
            &trust,
            signed.request,
            signed.metadata.metadata(),
            &signed.descriptor,
            NOW,
            context,
        )?;
        trust.remember_offer(&offer, context)?;
        drop((trust, offer));
        let trust = TrustStore::open(installation.path(), [92; 32], context)?;
        let restored = trust.cached_offer(signed.request, NOW, context)?.ok_or("Missing cache")?;
        assert_eq!(restored.offer().admission(), Admission::Prepare);
        assert_eq!(restored.authorization(&trust).descriptor, signed.descriptor);
        drop(restored);
        let mut current = signed.request;
        current.sequence_floor = 1;
        current.sequence_digest = Some(sha2::Sha256::digest(b"package").into());
        assert_eq!(
            trust.cached_offer(current, NOW, context)?.ok_or("Missing cache")?.offer().admission(),
            Admission::Current
        );
        let mut other = signed.request;
        other.channel = Channel::Canary;
        assert!(trust.cached_offer(other, NOW, context)?.is_none());
        trust.rotate(&root(&key(93)?, 2, Some(&key(92)?))?, context)?;
        assert!(trust.cached_offer(signed.request, NOW, context).is_err());
        Ok(())
    })
}

/// Locate the currently published generation without retaining a filesystem path as custody.
fn cached(
    trust: &TrustStore,
    request: &Request,
    context: &JobContext,
) -> Result<crate::setup_storage::cache::CacheLease, Box<dyn std::error::Error + Send + Sync>> {
    trust
        .runtime_cache(context)?
        .find(Namespace::UpdateOffers, &trust.offer_key(request), cache::utc_seconds()?, context)?
        .ok_or_else(|| "Missing cache".into())
}

#[test]
fn independent_publication_preserves_pinned_previous_bytes_and_ignores_staged_records() -> TestResult
{
    worker::run_reserved(crate::setup_storage::SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let installation = worker::installation(92, context)?;
        let signed = SignedPackage::new(b"package", 1)?;
        let trust = TrustStore::initialize(installation.path(), [92; 32], &signed.root, context)?;
        let offer = AuthenticatedOffer::authenticate(
            &trust,
            signed.request,
            signed.metadata.metadata(),
            &signed.descriptor,
            NOW,
            context,
        )?;
        trust.remember_offer(&offer, context)?;
        drop(offer);
        let previous = cached(&trust, &signed.request, context)?;
        let original = previous.read(RECORD, context)?.read_bounded(MAX_PAYLOAD_BYTES + 104)?;
        let next = SignedPackage::new(b"next", 2)?;
        let offer = AuthenticatedOffer::authenticate(
            &trust,
            next.request,
            next.metadata.metadata(),
            &next.descriptor,
            NOW,
            context,
        )?;
        trust.remember_offer(&offer, context)?;
        let current = cached(&trust, &next.request, context)?;
        assert_ne!(current.path(), previous.path());
        assert_eq!(
            previous.read(RECORD, context)?.read_bounded(MAX_PAYLOAD_BYTES + 104)?,
            original
        );
        assert_eq!(
            trust
                .cached_offer(next.request, NOW, context)?
                .ok_or("Missing offer")?
                .authorization(&trust)
                .descriptor,
            next.descriptor
        );
        let mut abandoned = trust.runtime_cache(context)?.fresh(
            Namespace::UpdateOffers,
            &trust.offer_key(&next.request),
            envelope::Kind::Offer.limit() as u64,
            cache::utc_seconds()?,
            context,
        )?;
        abandoned.write(RECORD, b"partial replacement", context)?;
        // Even a live producer of a newer generation cannot hide the complete published one.
        assert_eq!(
            trust
                .cached_offer(next.request, NOW, context)?
                .ok_or("Missing offer")?
                .authorization(&trust)
                .descriptor,
            next.descriptor
        );
        let abandoned_path = abandoned.path();
        drop(abandoned);
        assert_eq!(
            trust
                .cached_offer(next.request, NOW, context)?
                .ok_or("Missing offer")?
                .authorization(&trust)
                .descriptor,
            next.descriptor
        );
        assert_eq!(std::fs::read(abandoned_path.join(RECORD))?, b"partial replacement");
        assert!(trust.cached_offer(next.request, EXPIRES + 1, context).is_err());
        Ok(())
    })
}

#[test]
fn corrupt_foreign_or_unsigned_bytes_are_preserved_and_never_become_authority() -> TestResult {
    worker::run_reserved(crate::setup_storage::SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let installation = worker::installation(92, context)?;
        let signed = SignedPackage::new(b"package", 1)?;
        let trust = TrustStore::initialize(installation.path(), [92; 32], &signed.root, context)?;
        let offer = AuthenticatedOffer::authenticate(
            &trust,
            signed.request,
            signed.metadata.metadata(),
            &signed.descriptor,
            NOW,
            context,
        )?;
        trust.remember_offer(&offer, context)?;
        let lease = cached(&trust, &signed.request, context)?;
        let path = lease.path().join(RECORD);
        let original = std::fs::read(&path)?;
        drop(lease);
        std::fs::write(&path, b"corrupt")?;
        assert!(trust.cached_offer(signed.request, NOW, context).is_err());
        trust.remember_offer(&offer, context)?;
        assert_eq!(std::fs::read(&path)?, b"corrupt");
        assert!(trust.cached_offer(signed.request, NOW, context)?.is_some());
        let other = worker::installation(93, context)?;
        let other = TrustStore::initialize(other.path(), [92; 32], &signed.root, context)?;
        assert!(other.remember_offer(&offer, context).is_err());
        let mut foreign = other.runtime_cache(context)?.fresh(
            Namespace::UpdateOffers,
            &other.offer_key(&signed.request),
            envelope::Kind::Offer.limit() as u64,
            cache::utc_seconds()?,
            context,
        )?;
        foreign.write(RECORD, &original, context)?;
        foreign.publish(context)?;
        drop(foreign);
        assert!(other.cached_offer(signed.request, NOW, context).is_err());
        let lease = cached(&trust, &signed.request, context)?;
        let current = lease.path().join(RECORD);
        drop(lease);
        let bad = codec::encode(signed.metadata.metadata(), b"unsigned")?;
        let bad =
            envelope::encode(envelope::Kind::Offer, &trust.installation, &trust.repository, &bad)?;
        std::fs::write(&current, &bad)?;
        assert!(trust.cached_offer(signed.request, NOW, context).is_err());
        assert_eq!(std::fs::read(current)?, bad);
        Ok(())
    })
}

#[test]
fn disposable_offer_cleanup_preserves_trust_settings_and_legacy_cache_bytes() -> TestResult {
    worker::run_reserved(crate::setup_storage::SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let installation = worker::installation(92, context)?;
        let signed = SignedPackage::new(b"package", 1)?;
        let trust = TrustStore::initialize(installation.path(), [92; 32], &signed.root, context)?;
        let legacy = trust.root.path().join(format!("offer-v1-{}", signed.request.target_name()));
        std::fs::write(&legacy, b"legacy signed bytes")?;
        let settings = installation.path().join("user-settings");
        std::fs::write(&settings, b"authoritative settings")?;
        let offer = AuthenticatedOffer::authenticate(
            &trust,
            signed.request,
            signed.metadata.metadata(),
            &signed.descriptor,
            NOW,
            context,
        )?;
        trust.remember_offer(&offer, context)?;
        let checkpoint = std::fs::read(trust.root.path().join(super::super::RECORD))?;
        let report = trust
            .runtime_cache(context)?
            .maintain(cache::utc_seconds()? + Namespace::UpdateOffers.retention(), context)?;
        assert_eq!(report.removed_entries, 1);
        assert!(trust.cached_offer(signed.request, NOW, context)?.is_none());
        assert_eq!(std::fs::read(trust.root.path().join(super::super::RECORD))?, checkpoint);
        trust.offer(
            signed.metadata.metadata(),
            &signed.request,
            &signed.descriptor,
            NOW,
            context,
        )?;
        assert_eq!(std::fs::read(settings)?, b"authoritative settings");
        assert_eq!(std::fs::read(legacy)?, b"legacy signed bytes");
        Ok(())
    })
}

#[test]
fn cache_codec_preserves_signed_bytes_and_rejects_truncation_lengths_and_trailing_data()
-> TestResult {
    let signed = SignedPackage::new(b"package", 1)?;
    let bytes = codec::encode(signed.metadata.metadata(), &signed.descriptor)?;
    let decoded = codec::decode(&bytes, signed.request)?;
    assert_eq!(decoded.timestamp, signed.metadata.timestamp);
    assert_eq!(decoded.snapshot, signed.metadata.snapshot);
    assert_eq!(decoded.targets, signed.metadata.targets);
    assert_eq!(decoded.descriptor, signed.descriptor);
    for end in 0..bytes.len() {
        assert!(codec::decode(&bytes[..end], signed.request).is_err());
    }
    let mut bad = bytes.clone();
    bad.extend_from_slice(b"trailing");
    assert!(codec::decode(&bad, signed.request).is_err());
    for size in [0u32, u32::MAX] {
        bad[..4].copy_from_slice(&size.to_le_bytes());
        assert!(codec::decode(&bad, signed.request).is_err());
    }
    Ok(())
}

#[test]
fn unavailable_cache_preserves_independent_trust_initialization_and_rollback_floors() -> TestResult
{
    worker::run_reserved(crate::setup_storage::SIGNED_INSTALLER_RESERVATION_BYTES, |context, _| {
        let installation = worker::installation(92, context)?;
        let occupied = installation.path().join(cache::DIRECTORY);
        std::fs::write(&occupied, b"unrecognized cache-root bytes")?;
        let signed = SignedPackage::new(b"package", 1)?;
        let trust = TrustStore::initialize(installation.path(), [92; 32], &signed.root, context)?;
        let offer = AuthenticatedOffer::authenticate(
            &trust,
            signed.request,
            signed.metadata.metadata(),
            &signed.descriptor,
            NOW,
            context,
        )?;
        assert!(trust.remember_offer(&offer, context).is_err());
        let current =
            sugarsubstitute_update::fixture::Set::new(&key(92)?, [2; 3], b"new", EXPIRES)?;
        trust.refresh(current.metadata(), NOW, context)?;
        drop((offer, trust));
        let trust = TrustStore::open(installation.path(), [92; 32], context)?;
        assert!(matches!(
            trust.refresh(signed.metadata.metadata(), NOW, context),
            Err(TrustFailure::Verification(sugarsubstitute_update::Error::Rollback))
        ));
        assert_eq!(std::fs::read(occupied)?, b"unrecognized cache-root bytes");
        Ok(())
    })
}

#[test]
fn streamed_offer_matches_the_existing_envelope_at_maximum_metadata_bounds() -> TestResult {
    let metadata = vec![b'x'; sugarsubstitute_update::MAX_METADATA_BYTES];
    let descriptor = vec![b'd'; sugarsubstitute_contracts::update::MAX_OFFER_BYTES];
    let installation = InstallationId::new(format!("{:032x}", 92))?;
    let roles = || MetadataSet { timestamp: &metadata, snapshot: &metadata, targets: &metadata };
    let payload = codec::encode(roles(), &descriptor)?;
    let expected = envelope::encode(envelope::Kind::Offer, &installation, &[92; 32], &payload)?;
    let mut streamed = Vec::new();
    codec::write(&installation, &[92; 32], roles(), &descriptor, &mut streamed)?;
    assert_eq!(streamed, expected);
    assert_eq!(codec::record_len(&roles(), &descriptor)?, expected.len());
    assert_eq!(
        codec::decode(
            envelope::decode(envelope::Kind::Offer, &installation, &[92; 32], &streamed)?,
            SignedPackage::new(b"package", 1)?.request
        )?
        .descriptor,
        descriptor
    );
    Ok(())
}
