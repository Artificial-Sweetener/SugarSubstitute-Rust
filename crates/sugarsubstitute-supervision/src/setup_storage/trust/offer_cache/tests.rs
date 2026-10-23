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

#[test]
fn failed_cache_replacement_preserves_previous_bytes_and_ignores_staged_records() -> TestResult {
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
        let path = trust.root.path.join(name(&signed.request));
        let original = std::fs::read(&path)?;
        let orphan = trust.root.path.join("offer-pending-interrupted");
        std::fs::write(&orphan, b"partial")?;
        let pin = trust.root.read(&name(&signed.request))?;
        let next = SignedPackage::new(b"next", 2)?;
        let offer = AuthenticatedOffer::authenticate(
            &trust,
            next.request,
            next.metadata.metadata(),
            &next.descriptor,
            NOW,
            context,
        )?;
        assert!(trust.remember_offer(&offer, context).is_err());
        assert_eq!(std::fs::read(&path)?, original);
        drop(pin);
        trust.remember_offer(&offer, context)?;
        assert_eq!(std::fs::read(&orphan)?, b"partial");
        let current = std::fs::read(&path)?;
        let pin = trust.root.read(&name(&signed.request))?;
        trust.remember_offer(&offer, context)?;
        assert_eq!(std::fs::read(&path)?, current);
        drop(pin);
        drop(offer);
        assert!(trust.cached_offer(next.request, EXPIRES + 1, context).is_err());
        assert_eq!(std::fs::read(&path)?, current);
        Ok(())
    })
}

#[test]
fn corrupt_foreign_or_unsigned_cache_bytes_cannot_become_authority_or_be_overwritten() -> TestResult
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
        let path = trust.root.path.join(name(&signed.request));
        let original = std::fs::read(&path)?;
        std::fs::write(&path, b"corrupt")?;
        assert!(trust.cached_offer(signed.request, NOW, context).is_err());
        assert!(trust.remember_offer(&offer, context).is_err());
        assert_eq!(std::fs::read(&path)?, b"corrupt");
        std::fs::write(&path, &original)?;
        let other = worker::installation(93, context)?;
        let other = TrustStore::initialize(other.path(), [92; 32], &signed.root, context)?;
        other.protected(context, |protection| {
            protection
                .file(&other.root.path.join(name(&signed.request)), context)?
                .write_all(&original)?;
            Ok(())
        })?;
        assert!(other.cached_offer(signed.request, NOW, context).is_err());
        let bad = codec::encode(signed.metadata.metadata(), b"unsigned")?;
        let bad =
            envelope::encode(envelope::Kind::Offer, &trust.installation, &trust.repository, &bad)?;
        std::fs::write(&path, &bad)?;
        assert!(trust.cached_offer(signed.request, NOW, context).is_err());
        assert_eq!(std::fs::read(&path)?, bad);
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
        assert!(codec::validate(&bytes[..end]).is_err());
    }
    let mut bad = bytes.clone();
    bad.extend_from_slice(b"trailing");
    assert!(codec::validate(&bad).is_err());
    for size in [0u32, u32::MAX] {
        bad[..4].copy_from_slice(&size.to_le_bytes());
        assert!(codec::validate(&bad).is_err());
    }
    Ok(())
}

use sha2::Digest;
