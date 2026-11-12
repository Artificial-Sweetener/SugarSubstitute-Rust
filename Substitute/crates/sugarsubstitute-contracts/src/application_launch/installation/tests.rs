// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Explicit reader support and compiled engine identity govern durable installation activation.
use super::*;

#[test]
fn producing_engine_round_trip_preserves_the_entire_release_range() -> Result<(), Rejection> {
    let bytes = InstallationActivationContract::produced(u64::MAX, 7)?;
    let contract = InstallationActivationContract::decode(&bytes)?;
    contract.require_installation(AUTHORIZATION_SCHEMA, u64::MAX, 7)?;
    assert!(String::from_utf8_lossy(&bytes).contains("\"18446744073709551615\""));
    assert!(contract.require_installation(AUTHORIZATION_SCHEMA + 1, 1, 7).is_err());
    assert!(contract.require_installation(AUTHORIZATION_SCHEMA, 1, 8).is_err());
    Ok(())
}

#[test]
fn a_bundled_launcher_must_satisfy_the_authenticated_minimum() -> Result<(), Rejection> {
    let contract =
        InstallationActivationContract::decode(&InstallationActivationContract::produced(4, 1)?)?;
    contract.require_installation(AUTHORIZATION_SCHEMA, 4, 1)?;
    assert!(contract.require_installation(AUTHORIZATION_SCHEMA, 5, 1).is_err());
    assert!(contract.require_installation(AUTHORIZATION_SCHEMA, 0, 1).is_err());
    assert!(InstallationActivationContract::produced(0, 1).is_err());
    assert!(InstallationActivationContract::produced(1, 0).is_err());
    Ok(())
}

#[test]
fn historical_accepted_launcher_metadata_does_not_grant_installation_activation() {
    assert!(
        InstallationActivationContract::decode(super::super::CURRENT_LAUNCHER_CONTRACT).is_err()
    );
}

#[test]
fn malformed_future_and_ambiguous_declarations_are_rejected()
-> Result<(), Box<dyn std::error::Error>> {
    let original = InstallationActivationContract::produced(1, 1)?;
    for (key, value) in [
        ("schema_version", serde_json::json!(2)),
        ("authorization_schemas", serde_json::json!([])),
        ("authorization_schemas", serde_json::json!([0])),
        ("authorization_schemas", serde_json::json!([1, 1])),
        ("authorization_schemas", serde_json::json!([2, 1])),
        ("engine_release", serde_json::json!("01")),
        ("engine_release", serde_json::json!("+1")),
        ("engine_release", serde_json::json!("18446744073709551616")),
        ("engine_release", serde_json::json!(1)),
        ("engine_protocol", serde_json::json!(0)),
        ("unknown", serde_json::json!(true)),
    ] {
        let mut changed: serde_json::Value = serde_json::from_slice(&original)?;
        changed[key] = value;
        assert!(InstallationActivationContract::decode(&serde_json::to_vec(&changed)?).is_err());
    }
    let duplicate = String::from_utf8(original)?.replace("{", "{\"schema_version\":1,");
    assert!(InstallationActivationContract::decode(duplicate.as_bytes()).is_err());
    assert!(InstallationActivationContract::decode(&vec![b' '; MAX_CONTRACT_BYTES + 1]).is_err());
    Ok(())
}
