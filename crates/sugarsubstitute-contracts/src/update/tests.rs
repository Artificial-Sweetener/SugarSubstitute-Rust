// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Product routing and compatibility decisions without network or installation side effects.
use super::*;
use serde_json::{Value, json};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn wire() -> Value {
    json!({"schema_version":1,"product":"sugarsubstitute","channel":"release",
        "platform":"windows-x86_64","component":"application","sequence":2,"version":"2.0.0",
        "package_sha256":"a".repeat(64),"package_bytes":123,"package_schema":1,
        "updater_protocol":1,"minimum_updater_release":1})
}
fn offer(value: &Value) -> Result<Offer, Rejection> {
    Offer::decode(value.to_string().as_bytes())
}
fn request() -> Request {
    Request {
        channel: Channel::default(),
        platform: Platform::WindowsX86_64,
        component: Component::Application,
        sequence_floor: 1,
        sequence_digest: Some([0xaa; 32]),
        updater_release: 1,
        updater_protocol: 1,
        package_schema: 1,
        intent: PreparationIntent::Automatic,
    }
}

#[test]
fn release_is_default_and_channels_platforms_and_components_do_not_cross() -> TestResult {
    let request = request();
    assert_eq!(request.channel, Channel::Release);
    assert_eq!(request.target_name(), "application-release-windows-x86_64.json");
    assert_eq!(offer(&wire())?.admission(&request)?, Admission::Prepare);
    for (field, value) in [
        ("channel", "canary"),
        ("platform", "macos-aarch64"),
        ("component", "updater"),
        ("product", "another-product"),
    ] {
        let mut data = wire();
        data[field] = value.into();
        assert_eq!(offer(&data)?.admission(&request), Err(Rejection::Routing));
    }
    let mut data = wire();
    data["channel"] = "canary".into();
    let mut opted_in = request;
    opted_in.channel = Channel::Canary;
    assert_eq!(offer(&data)?.admission(&opted_in)?, Admission::Prepare);
    Ok(())
}

#[test]
fn old_engine_gets_explicit_upgrade_decision_before_package_preparation() -> TestResult {
    let mut data = wire();
    data["minimum_updater_release"] = 2.into();
    assert_eq!(offer(&data)?.minimum_updater_release(), 2);
    assert_eq!(offer(&data)?.admission(&request())?, Admission::UpdateEngine);
    data["minimum_updater_release"] = 1.into();
    data["package_schema"] = 2.into();
    assert_eq!(offer(&data)?.admission(&request())?, Admission::Installer);
    data["package_schema"] = 1.into();
    data["updater_protocol"] = 2.into();
    assert_eq!(offer(&data)?.admission(&request())?, Admission::Installer);
    Ok(())
}

#[test]
fn repair_retries_current_identity_but_never_downgrades_or_relabels_it() -> TestResult {
    let mut request = request();
    request.sequence_floor = 2;
    assert_eq!(offer(&wire())?.admission(&request)?, Admission::Current);
    request.intent = PreparationIntent::Repair;
    assert_eq!(offer(&wire())?.admission(&request)?, Admission::Prepare);
    let mut data = wire();
    data["sequence"] = 1.into();
    assert_eq!(offer(&data)?.admission(&request), Err(Rejection::Rollback));
    data["sequence"] = 2.into();
    data["package_sha256"] = "b".repeat(64).into();
    assert_eq!(offer(&data)?.admission(&request), Err(Rejection::Identity));
    request.sequence_digest = None;
    assert_eq!(offer(&wire())?.admission(&request), Err(Rejection::LocalState));
    Ok(())
}

#[test]
fn descriptor_rejects_unknown_fields_bad_digests_and_unbounded_input() {
    for (field, value) in [
        ("schema_version", json!(2)),
        ("sequence", json!(0)),
        ("package_bytes", json!(0)),
        ("package_sha256", json!("A".repeat(64))),
        ("version", json!("<script>")),
        ("download_url", json!("https://unknown/evil.exe")),
    ] {
        let mut data = wire();
        data[field] = value;
        assert!(offer(&data).is_err());
    }
    assert!(matches!(Offer::decode(&vec![0; MAX_OFFER_BYTES + 1]), Err(Rejection::Bound)));
}

#[test]
fn every_platform_has_one_canonical_signed_target_spelling() -> TestResult {
    let mut request = request();
    for platform in [
        Platform::WindowsX86_64,
        Platform::WindowsAarch64,
        Platform::MacosX86_64,
        Platform::MacosAarch64,
        Platform::LinuxX86_64,
        Platform::LinuxAarch64,
    ] {
        assert_eq!(serde_json::to_value(platform)?, platform.as_str());
        request.platform = platform;
        assert!(request.target_name().ends_with(&format!("-{}.json", platform.as_str())));
    }
    Ok(())
}
