// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! An expired embedded release verifies only through genuine root-authorised signatures.
use super::*;
use crate::fixture::*;
use serde_json::json;
use sha2::{Digest, Sha256};
use sugarsubstitute_contracts::{
    installation::PreparationIntent,
    update::{Channel, Component, Platform},
};
use tuf::crypto::Ed25519PrivateKey;
type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

const PAYLOAD: &[u8] = b"embedded installer package";

fn request() -> Request {
    Request {
        channel: Channel::Release,
        platform: Platform::WindowsX86_64,
        component: Component::Application,
        sequence_floor: 0,
        sequence_digest: None,
        updater_release: 1,
        updater_protocol: 1,
        package_schema: 1,
        intent: PreparationIntent::Repair,
    }
}

/// Signed set naming `PAYLOAD`, expiring at `expires`, and its descriptor bytes.
fn embedded(
    key: &Ed25519PrivateKey,
    expires: i64,
) -> Result<(Set, Vec<u8>), Box<dyn std::error::Error + Send + Sync>> {
    let digest = format!("{:x}", Sha256::digest(PAYLOAD));
    let descriptor = serde_json::to_vec(&json!({"schema_version":1,"product":"sugarsubstitute",
        "channel":"release","platform":"windows-x86_64","component":"application","sequence":4,
        "version":"4.0.0","package_sha256":digest,"package_bytes":PAYLOAD.len(),
        "package_schema":1,"updater_protocol":1,"minimum_updater_release":1}))?;
    let package = format!("{digest}.ssupdate");
    let name = request().target_name();
    let set = Set::targets(
        key,
        [1; 3],
        &[(name.as_str(), descriptor.as_slice()), (package.as_str(), PAYLOAD)],
        expires,
    )?;
    Ok((set, descriptor))
}

#[test]
fn an_expired_embedded_release_verifies_pinned_and_only_its_exact_bytes_pass() -> TestResult {
    let key = key(71)?;
    let trust = RepositoryTrust::bootstrap(&root(&key, 1, None)?)?;
    let (set, descriptor) = embedded(&key, NOW + 10)?;
    let later = EXPIRES + 365 * 24 * 60 * 60;
    let mut current = RepositoryTrust::bootstrap(&root(&key, 1, None)?)?;
    assert!(current.refresh(set.metadata(), later).is_err_and(|cause| cause.is_expiry()));
    let checkpoint = trust.checkpoint()?;
    let offer = trust.pinned_offer(set.metadata(), &request(), &descriptor)?;
    assert_eq!(offer.offer().sequence(), 4);
    assert_eq!(trust.checkpoint()?, checkpoint);
    let mut exact = offer.into_package()?.verifier(later)?;
    exact.push(PAYLOAD)?;
    assert_eq!(exact.finish(later)?.digest(), <[u8; 32]>::from(Sha256::digest(PAYLOAD)));
    let offer = trust.pinned_offer(set.metadata(), &request(), &descriptor)?;
    let mut tampered = offer.into_package()?.verifier(later)?;
    tampered.push(b"embedded installer packagf")?;
    assert!(matches!(tampered.finish(later), Err(Error::Digest)));
    Ok(())
}

#[test]
fn metadata_signed_by_a_key_the_root_does_not_authorise_is_refused() -> TestResult {
    let trust = RepositoryTrust::bootstrap(&root(&key(72)?, 1, None)?)?;
    let (forged, descriptor) = embedded(&key(73)?, NOW + 10)?;
    assert!(trust.pinned_offer(forged.metadata(), &request(), &descriptor).is_err());
    Ok(())
}

#[test]
fn a_changed_descriptor_or_role_is_refused() -> TestResult {
    let key = key(74)?;
    let trust = RepositoryTrust::bootstrap(&root(&key, 1, None)?)?;
    let (set, mut descriptor) = embedded(&key, NOW + 10)?;
    let mut changed = descriptor.clone();
    changed[0] = b' ';
    assert!(trust.pinned_offer(set.metadata(), &request(), &changed).is_err());
    let (mut altered, _) = embedded(&key, NOW + 10)?;
    let last = altered.targets.len() - 2;
    altered.targets[last] ^= 1;
    assert!(trust.pinned_offer(altered.metadata(), &request(), &descriptor).is_err());
    descriptor.truncate(descriptor.len() - 1);
    assert!(trust.pinned_offer(set.metadata(), &request(), &descriptor).is_err());
    Ok(())
}

#[test]
fn a_rotated_root_refuses_metadata_signed_by_the_retired_key() -> TestResult {
    let (old, new) = (key(75)?, key(76)?);
    let mut trust = RepositoryTrust::bootstrap(&root(&old, 1, None)?)?;
    trust.rotate(&root(&new, 2, Some(&old))?)?;
    let (set, descriptor) = embedded(&old, NOW + 10)?;
    assert!(trust.pinned_offer(set.metadata(), &request(), &descriptor).is_err());
    let (set, descriptor) = embedded(&new, NOW + 10)?;
    trust.pinned_offer(set.metadata(), &request(), &descriptor)?;
    Ok(())
}

#[test]
fn lifetime_end_and_role_names_read_the_compiled_in_set() -> TestResult {
    let key = key(77)?;
    let (set, _) = embedded(&key, NOW + 10)?;
    assert_eq!(signed_lifetime_end(set.metadata(), &root(&key, 1, None)?)?, NOW + 10);
    assert_eq!(named_snapshot(&set.timestamp)?, 1);
    assert_eq!(named_targets(&set.snapshot)?, 1);
    assert!(named_snapshot(b"").is_err());
    assert!(named_targets(b"not metadata").is_err());
    Ok(())
}
