// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Durable floors admit only authenticated route identities and never claim application health.
use super::*;
use crate::{RepositoryTrust, fixture::*};
use sha2::{Digest, Sha256};
use sugarsubstitute_contracts::installation::PreparationIntent;
type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

fn request() -> Request {
    Request {
        channel: Channel::Release,
        component: Component::Application,
        platform: Platform::WindowsX86_64,
        sequence_floor: 0,
        sequence_digest: None,
        updater_release: 1,
        updater_protocol: 1,
        package_schema: 1,
        intent: PreparationIntent::Automatic,
    }
}
fn offer(
    request: &Request,
    sequence: u64,
    payload: &[u8],
) -> Result<VerifiedOffer, Box<dyn std::error::Error + Send + Sync>> {
    let digest = format!("{:x}", Sha256::digest(payload));
    let descriptor = serde_json::to_vec(&serde_json::json!({
        "schema_version":1,"product":"sugarsubstitute","channel":request.channel,
        "platform":request.platform,"component":request.component,"sequence":sequence,
        "version":"1.0.0","package_sha256":digest,"package_bytes":payload.len(),
        "package_schema":1,"updater_protocol":1,"minimum_updater_release":1,
    }))?;
    let key = key(96)?;
    let mut trust = RepositoryTrust::bootstrap(&root(&key, 1, None)?)?;
    let metadata = Set::targets(
        &key,
        [1; 3],
        &[(&request.target_name(), &descriptor), (&format!("{digest}.ssupdate"), payload)],
        EXPIRES,
    )?;
    trust.refresh(metadata.metadata(), NOW)?;
    Ok(trust.offer(request, &descriptor, NOW)?)
}

#[test]
fn reopened_history_rejects_downgrade_and_sequence_reuse_with_empty_caller_history() -> TestResult {
    let mut request = request();
    let current = offer(&request, 8, b"current")?;
    let history = ReleaseHistory::empty().authorize(&request, &current)?;
    let bytes = history.encode()?;
    let restored = ReleaseHistory::decode(&bytes)?;
    for intent in [PreparationIntent::Automatic, PreparationIntent::Repair] {
        request.intent = intent;
        assert!(matches!(
            restored.authorize(&request, &offer(&request, 7, b"older")?),
            Err(Error::Offer(Rejection::Rollback))
        ));
        assert!(matches!(
            restored.authorize(&request, &offer(&request, 8, b"substituted")?),
            Err(Error::Offer(Rejection::Identity))
        ));
        assert_eq!(restored.authorize(&request, &current)?.encode()?, bytes);
    }
    // Admitted is not installed or healthy: the same package can finish its interrupted attempt.
    assert_eq!(current.admission(), Admission::Prepare);
    assert_eq!(restored.encode()?, bytes);
    Ok(())
}

#[test]
fn channels_components_and_platforms_keep_independent_monotonic_floors() -> TestResult {
    let mut history = ReleaseHistory::empty();
    let mut sequence = 1;
    for channel in [Channel::Release, Channel::Canary] {
        for component in [Component::Application, Component::Updater] {
            for platform in [
                Platform::WindowsX86_64,
                Platform::WindowsAarch64,
                Platform::MacosX86_64,
                Platform::MacosAarch64,
                Platform::LinuxX86_64,
                Platform::LinuxAarch64,
            ] {
                let request = Request { channel, component, platform, ..request() };
                let offered = offer(&request, sequence, b"package")?;
                history = history.authorize(&request, &offered)?;
                sequence += 1;
            }
        }
    }
    let restored = ReleaseHistory::decode(&history.encode()?)?;
    assert_eq!(restored.floors.len(), MAX_ROUTES);
    let release = request();
    restored.check(&release, &offer(&release, 1, b"package")?)?;
    let canary = Request { channel: Channel::Canary, ..request() };
    assert!(matches!(
        restored.check(&canary, &offer(&canary, 1, b"package")?),
        Err(Error::Offer(Rejection::Rollback))
    ));
    Ok(())
}

#[test]
fn damaged_unknown_and_incomplete_history_never_becomes_empty_trust() -> TestResult {
    let request = request();
    let history = ReleaseHistory::empty().authorize(&request, &offer(&request, 2, b"payload")?)?;
    let bytes = history.encode()?;
    let original: serde_json::Value = serde_json::from_slice(&bytes)?;
    let mut duplicate = original.clone();
    let floor = duplicate["floors"][0].clone();
    duplicate["floors"].as_array_mut().ok_or("Missing floors")?.push(floor);
    let mut zero = original.clone();
    zero["floors"][0]["sequence"] = 0.into();
    let mut future = original.clone();
    future["schema_version"] = 2.into();
    let mut unknown = original;
    unknown["assume_current"] = true.into();
    for value in [
        duplicate,
        zero,
        future,
        unknown,
        serde_json::json!({"schema_version":1}),
        serde_json::json!({"schema_version":1,"floors":null}),
    ] {
        assert!(ReleaseHistory::decode(&serde_json::to_vec(&value)?).is_err());
    }
    assert!(ReleaseHistory::decode(&vec![b' '; MAX_RELEASE_HISTORY_BYTES + 1]).is_err());
    assert_eq!(history.encode()?, bytes);
    Ok(())
}

#[test]
fn incompatible_or_rebound_offer_cannot_advance_publication_history() -> TestResult {
    let request = request();
    let history = ReleaseHistory::empty();
    let incompatible = Request { package_schema: 2, ..self::request() };
    assert!(matches!(
        history.authorize(&incompatible, &offer(&incompatible, 9, b"package")?),
        Err(Error::Admission)
    ));
    let current = offer(&request, 9, b"package")?;
    let other = Request { channel: Channel::Canary, ..self::request() };
    assert!(matches!(history.authorize(&other, &current), Err(Error::Offer(Rejection::Routing))));
    assert_eq!(history.encode()?, ReleaseHistory::empty().encode()?);
    Ok(())
}
