// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Signed channel descriptors cannot substitute another subscription, component or package.
use super::*;
use serde_json::json;
use sha2::{Digest, Sha256};
use sugarsubstitute_contracts::{
    installation::PreparationIntent,
    update::{Admission, Channel, Component, Platform, Request},
};

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
fn descriptor(payload: &[u8]) -> serde_json::Value {
    json!({"schema_version":1,"product":"sugarsubstitute","channel":"release",
        "platform":"windows-x86_64","component":"application","sequence":2,"version":"2.0.0",
        "package_sha256":format!("{:x}", Sha256::digest(payload)),"package_bytes":payload.len(),
        "package_schema":1,"updater_protocol":1,"minimum_updater_release":1})
}
fn signed_offer(
    value: &serde_json::Value,
    payload: Option<&[u8]>,
) -> Result<(RepositoryTrust, Vec<u8>), Box<dyn std::error::Error + Send + Sync>> {
    let key = key(22)?;
    let descriptor = serde_json::to_vec(value)?;
    let name = request().target_name();
    let package =
        format!("{}.ssupdate", value["package_sha256"].as_str().ok_or("Missing test digest")?);
    let mut entries = vec![(name.as_str(), descriptor.as_slice())];
    if let Some(payload) = payload {
        entries.push((&package, payload));
    }
    let metadata = Set::targets(&key, [1; 3], &entries, EXPIRES)?;
    let mut trust = RepositoryTrust::bootstrap(&root(&key, 1, None)?)?;
    trust.refresh(metadata.metadata(), NOW)?;
    Ok((trust, descriptor))
}

#[test]
fn descriptor_and_package_must_share_one_complete_authenticated_repository() -> TestResult {
    let payload = b"native update package";
    let (trust, descriptor) = signed_offer(&descriptor(payload), Some(payload))?;
    let offer = trust.offer(&request(), &descriptor, NOW)?;
    assert_eq!(offer.admission(), Admission::Prepare);
    assert_eq!(offer.offer().version(), "2.0.0");
    assert_eq!(offer.offer().sequence(), 2);
    let package = offer.into_package()?;
    assert_eq!(package.name(), format!("{:x}.ssupdate", Sha256::digest(payload)));
    let mut verifier = package.verifier(NOW)?;
    verifier.push(payload)?;
    verifier.finish(NOW)?;
    Ok(())
}

#[test]
fn authenticated_descriptor_cannot_substitute_unbound_package_bytes() -> TestResult {
    let value = descriptor(b"wanted");
    let (trust, bytes) = signed_offer(&value, Some(b"wrong!"))?;
    assert!(matches!(trust.offer(&request(), &bytes, NOW), Err(Error::Digest)));
    let (trust, bytes) = signed_offer(&value, None)?;
    assert!(matches!(trust.offer(&request(), &bytes, NOW), Err(Error::Incomplete)));
    let (trust, mut bytes) = signed_offer(&value, Some(b"wanted"))?;
    bytes.push(b' ');
    assert!(trust.offer(&request(), &bytes, NOW).is_err());
    Ok(())
}

#[test]
fn even_valid_signatures_cannot_route_canary_to_release_users() -> TestResult {
    let payload = b"canary payload";
    let mut value = descriptor(payload);
    value["channel"] = "canary".into();
    let (trust, bytes) = signed_offer(&value, Some(payload))?;
    assert!(matches!(
        trust.offer(&request(), &bytes, NOW),
        Err(Error::Offer(sugarsubstitute_contracts::update::Rejection::Routing))
    ));
    Ok(())
}

#[test]
fn engine_upgrade_requirement_blocks_package_verification_ticket() -> TestResult {
    let payload = b"future package";
    let mut value = descriptor(payload);
    value["minimum_updater_release"] = 2.into();
    let (trust, bytes) = signed_offer(&value, Some(payload))?;
    let offer = trust.offer(&request(), &bytes, NOW)?;
    assert_eq!(offer.admission(), Admission::UpdateEngine);
    assert!(matches!(offer.into_package(), Err(Error::Admission)));
    Ok(())
}
