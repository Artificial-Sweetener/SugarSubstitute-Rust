// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Each D22 condition refuses on its own; a fresh receipt starts offline once metadata expired.
use super::*;
use crate::fixture::*;
use serde_json::json;
use sugarsubstitute_contracts::installation::PreparationIntent;
use tuf::crypto::Ed25519PrivateKey;
type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

const DAY: i64 = 24 * 60 * 60;

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
        intent: PreparationIntent::Automatic,
    }
}

/// Signed descriptor bytes for `sequence` and the package name it binds.
fn release(sequence: u64, payload: &[u8]) -> Result<(Vec<u8>, String), serde_json::Error> {
    let digest = format!("{:x}", Sha256::digest(payload));
    let descriptor = serde_json::to_vec(&json!({"schema_version":1,"product":"sugarsubstitute",
        "channel":"release","platform":"windows-x86_64","component":"application",
        "sequence":sequence,"version":format!("{sequence}.0.0"),"package_sha256":digest,
        "package_bytes":payload.len(),"package_schema":1,"updater_protocol":1,
        "minimum_updater_release":1}))?;
    Ok((descriptor, format!("{digest}.ssupdate")))
}

/// Signed targets listing one release, or only an unrelated target when `None`.
fn listing(
    key: &Ed25519PrivateKey,
    versions: [u32; 3],
    listed: Option<(u64, &[u8])>,
) -> Result<(Set, Vec<u8>), Box<dyn std::error::Error + Send + Sync>> {
    let name = request().target_name();
    let Some((sequence, payload)) = listed else {
        return Ok((Set::targets(key, versions, &[("unrelated.json", b"{}")], EXPIRES)?, vec![]));
    };
    let (descriptor, package) = release(sequence, payload)?;
    let set = Set::targets(
        key,
        versions,
        &[(name.as_str(), descriptor.as_slice()), (package.as_str(), payload)],
        EXPIRES,
    )?;
    Ok((set, descriptor))
}

/// A trust that authenticated release 2, its receipt and its durable history.
struct Prepared {
    key: Ed25519PrivateKey,
    trust: RepositoryTrust,
    release: PreparedRelease,
    history: ReleaseHistory,
}
impl Prepared {
    fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let key = key(61)?;
        let mut trust = RepositoryTrust::bootstrap(&root(&key, 1, None)?)?;
        let (set, descriptor) = listing(&key, [1; 3], Some((2, b"package two")))?;
        trust.refresh(set.metadata(), NOW)?;
        let offer = trust.offer(&request(), &descriptor, NOW)?;
        let release = trust.prepared_release(&request(), &offer, &descriptor, NOW)?;
        let history = ReleaseHistory::empty().authorize(&request(), &offer)?;
        // Later launches restore protected trust; no complete signed set is verified offline.
        let trust = RepositoryTrust::from_trusted_checkpoint(&trust.checkpoint()?)?;
        Ok(Self { key, trust, release, history })
    }
    fn admit(&self, newest: Option<&[u8]>, now: i64) -> Result<(), ReceiptRefusal> {
        self.trust.admit_prepared(&self.release, &request(), &self.history, newest, now)
    }
}

#[test]
fn receipt_records_the_exact_authenticated_release_root_and_targets() -> TestResult {
    let prepared = Prepared::new()?;
    let (descriptor, package) = release(2, b"package two")?;
    assert_eq!(prepared.release.sequence, 2);
    assert_eq!(prepared.release.package_name, package);
    assert_eq!(prepared.release.package_digest, <[u8; 32]>::from(Sha256::digest(b"package two")));
    assert_eq!(prepared.release.descriptor_digest, <[u8; 32]>::from(Sha256::digest(descriptor)));
    assert_eq!((prepared.release.root_version, prepared.release.targets_version), (1, 1));
    assert_eq!(prepared.release.received_at, NOW);
    Ok(())
}

#[test]
fn a_fresh_receipt_activates_offline_after_its_cached_metadata_expired() -> TestResult {
    let prepared = Prepared::new()?;
    let later = EXPIRES + 30 * DAY;
    let (set, descriptor) = listing(&prepared.key, [1; 3], Some((2, b"package two")))?;
    let mut expired = RepositoryTrust::from_trusted_checkpoint(&prepared.trust.checkpoint()?)?;
    assert!(expired.refresh(set.metadata(), later).is_err());
    assert!(expired.offer(&request(), &descriptor, later).is_err());
    prepared.admit(None, later)?;
    Ok(())
}

#[test]
fn a_release_below_the_installation_floor_is_refused() -> TestResult {
    let mut prepared = Prepared::new()?;
    let mut trust = RepositoryTrust::from_trusted_checkpoint(&prepared.trust.checkpoint()?)?;
    let (set, descriptor) = listing(&prepared.key, [2; 3], Some((3, b"package three")))?;
    trust.refresh(set.metadata(), NOW + 1)?;
    prepared.history =
        prepared.history.authorize(&request(), &trust.offer(&request(), &descriptor, NOW + 1)?)?;
    assert_eq!(prepared.admit(Some(&set.targets), NOW + 2), Err(ReceiptRefusal::BelowFloor));
    let healthy = Request { sequence_floor: 3, ..request() };
    let fresh = Prepared::new()?;
    assert_eq!(
        fresh.trust.admit_prepared(&fresh.release, &healthy, &fresh.history, None, NOW),
        Err(ReceiptRefusal::BelowFloor)
    );
    Ok(())
}

#[test]
fn a_root_rotation_since_the_receipt_requires_an_online_refresh() -> TestResult {
    let mut prepared = Prepared::new()?;
    prepared.trust.rotate(&root(&prepared.key, 2, Some(&prepared.key))?)?;
    assert_eq!(prepared.admit(None, NOW), Err(ReceiptRefusal::RootAdvanced));
    Ok(())
}

#[test]
fn newer_metadata_that_omits_the_release_revokes_it() -> TestResult {
    let mut prepared = Prepared::new()?;
    let (set, _) = listing(&prepared.key, [2; 3], None)?;
    prepared.trust.refresh(set.metadata(), NOW + 1)?;
    assert_eq!(prepared.admit(Some(&set.targets), NOW + 2), Err(ReceiptRefusal::Revoked));
    Ok(())
}

#[test]
fn newer_metadata_that_names_another_release_supersedes_it() -> TestResult {
    let mut prepared = Prepared::new()?;
    let (set, _) = listing(&prepared.key, [2; 3], Some((3, b"package three")))?;
    prepared.trust.refresh(set.metadata(), NOW + 1)?;
    assert_eq!(prepared.admit(Some(&set.targets), NOW + 2), Err(ReceiptRefusal::Superseded));
    Ok(())
}

#[test]
fn newer_metadata_that_cannot_be_inspected_refuses() -> TestResult {
    let mut prepared = Prepared::new()?;
    let (set, _) = listing(&prepared.key, [2; 3], Some((2, b"package two")))?;
    prepared.trust.refresh(set.metadata(), NOW + 1)?;
    let (older, _) = listing(&prepared.key, [1; 3], Some((2, b"package two")))?;
    for newest in [None, Some(older.targets.as_slice()), Some(b"forged".as_slice())] {
        assert_eq!(prepared.admit(newest, NOW + 2), Err(ReceiptRefusal::NewerMetadataUnavailable));
    }
    let mut announced = Prepared::new()?;
    let (mut unfetched, _) = listing(&announced.key, [3, 3, 3], Some((2, b"package two")))?;
    unfetched.targets.clear();
    assert!(announced.trust.refresh(unfetched.metadata(), NOW + 1).is_err());
    assert_eq!(announced.admit(None, NOW + 2), Err(ReceiptRefusal::NewerMetadataUnavailable));
    Ok(())
}

#[test]
fn newer_metadata_that_still_lists_the_same_release_admits_it() -> TestResult {
    let mut prepared = Prepared::new()?;
    let (set, _) = listing(&prepared.key, [2; 3], Some((2, b"package two")))?;
    prepared.trust.refresh(set.metadata(), NOW + 1)?;
    prepared.admit(Some(&set.targets), EXPIRES + DAY)?;
    Ok(())
}

#[test]
fn a_receipt_older_than_ninety_days_is_refused() -> TestResult {
    let prepared = Prepared::new()?;
    prepared.admit(None, NOW + RECEIPT_LIFETIME_SECONDS)?;
    assert_eq!(prepared.admit(None, NOW + RECEIPT_LIFETIME_SECONDS + 1), Err(ReceiptRefusal::Aged));
    assert_eq!(RECEIPT_LIFETIME_SECONDS, 90 * DAY);
    Ok(())
}

#[test]
fn a_clock_before_the_receipt_or_the_last_verification_is_refused() -> TestResult {
    let mut prepared = Prepared::new()?;
    assert_eq!(prepared.admit(None, NOW - 1), Err(ReceiptRefusal::ClockRollback));
    let (set, _) = listing(&prepared.key, [2; 3], Some((2, b"package two")))?;
    prepared.trust.refresh(set.metadata(), NOW + 100)?;
    assert_eq!(prepared.admit(Some(&set.targets), NOW + 99), Err(ReceiptRefusal::ClockRollback));
    Ok(())
}

#[test]
fn a_receipt_for_another_route_is_refused() -> TestResult {
    let prepared = Prepared::new()?;
    let canary = Request { channel: Channel::Canary, ..request() };
    assert_eq!(
        prepared.trust.admit_prepared(&prepared.release, &canary, &prepared.history, None, NOW),
        Err(ReceiptRefusal::Route)
    );
    Ok(())
}

#[test]
fn receipt_facts_ignore_fields_added_by_a_newer_writer() -> TestResult {
    let prepared = Prepared::new()?;
    let mut value = serde_json::to_value(&prepared.release)?;
    value["added_later"] = json!({"any": "shape"});
    let decoded: PreparedRelease = serde_json::from_value(value)?;
    assert_eq!(decoded, prepared.release);
    Ok(())
}
