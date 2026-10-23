// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Updater entry compatibility is independent of application and launcher capabilities.
use super::*;
use serde_json::json;

#[test]
fn producing_updater_declares_exactly_the_implemented_entry_and_reader() -> Result<(), Rejection> {
    let contract = UpdaterContract::decode(CURRENT_UPDATER_CONTRACT)?;
    assert_eq!(contract.preparation_protocols, [PREPARATION_PROTOCOL]);
    assert_eq!(contract.installation_schemas, [crate::installation::SCHEMA_VERSION]);
    assert_eq!(contract.probe_protocols, [probe::PROBE_PROTOCOL]);
    contract.require_probe(probe::PROBE_PROTOCOL, crate::installation::SCHEMA_VERSION)?;
    contract.require_preparation(PREPARATION_PROTOCOL, crate::installation::SCHEMA_VERSION)
}

#[test]
fn historical_preparation_declarations_do_not_gain_probe_capability() -> Result<(), Rejection> {
    let bytes = br#"{"schema_version":1,"preparation_protocols":[1],"installation_schemas":[1]}"#;
    let old = UpdaterContract::decode(bytes)?;
    old.require_preparation(1, 1)?;
    assert_eq!(old.require_probe(probe::PROBE_PROTOCOL, 1), Err(Rejection::Unsupported));
    let historical_probe = UpdaterContract::decode(br#"{"schema_version":2,"preparation_protocols":[1],"installation_schemas":[1],"probe_protocols":[1]}"#)?;
    assert_eq!(
        historical_probe.require_probe(probe::PROBE_PROTOCOL, 1),
        Err(Rejection::Unsupported)
    );
    let current = UpdaterContract::decode(CURRENT_UPDATER_CONTRACT)?;
    assert_eq!(current.require_probe(1, 1), Err(Rejection::Unsupported));
    assert_eq!(current.require_probe(3, 1), Err(Rejection::Unsupported));
    assert_eq!(current.require_probe(1, 2), Err(Rejection::InstallationSchema));
    Ok(())
}

#[test]
fn probe_declarations_require_the_exact_schema_and_nonempty_ordered_versions() {
    for versions in [json!([]), json!([0]), json!([1, 1]), json!([2, 1]), json!(null)] {
        let value = json!({"schema_version":2,"preparation_protocols":[1],"installation_schemas":[1],"probe_protocols":versions});
        assert!(UpdaterContract::decode(value.to_string().as_bytes()).is_err());
    }
    for text in [
        r#"{"schema_version":1,"preparation_protocols":[1],"installation_schemas":[1],"probe_protocols":[1]}"#,
        r#"{"schema_version":2,"preparation_protocols":[1],"installation_schemas":[1],"probe_protocols":[1],"probe_protocols":[2]}"#,
    ] {
        assert!(UpdaterContract::decode(text.as_bytes()).is_err());
    }
}

#[test]
fn future_entries_and_installation_readers_require_explicit_support() -> Result<(), Rejection> {
    let contract = UpdaterContract::decode(CURRENT_UPDATER_CONTRACT)?;
    assert_eq!(contract.require_preparation(2, 1), Err(Rejection::Unsupported));
    assert_eq!(contract.require_preparation(1, 2), Err(Rejection::InstallationSchema));
    let transition = UpdaterContract::decode(
        br#"{"schema_version":1,"preparation_protocols":[1,2],"installation_schemas":[1,2]}"#,
    )?;
    transition.require_preparation(1, 1)?;
    transition.require_preparation(2, 2)
}

#[test]
fn malformed_metadata_never_claims_an_updater_entry() {
    for field in ["preparation_protocols", "installation_schemas"] {
        for versions in [
            json!([]),
            json!([0]),
            json!([1, 1]),
            json!([2, 1]),
            json!([1.5]),
            json!((1..=17).collect::<Vec<_>>()),
        ] {
            let mut value =
                json!({"schema_version":1,"preparation_protocols":[1],"installation_schemas":[1]});
            value[field] = versions;
            assert_eq!(
                UpdaterContract::decode(value.to_string().as_bytes()).err(),
                Some(Rejection::Format)
            );
        }
    }
    for bytes in [
        br#"{"schema_version":2,"preparation_protocols":[1],"installation_schemas":[1]}"#.as_slice(),
        br#"{"schema_version":1,"preparation_protocols":[1],"installation_schemas":[1],"command":"foreign.exe"}"#,
        br#"{"schema_version":1,"preparation_protocols":[1],"preparation_protocols":[2],"installation_schemas":[1]}"#,
        br#"{"schema_version":1,"preparation_protocols":[1]}"#,
    ] {
        assert_eq!(UpdaterContract::decode(bytes).err(), Some(Rejection::Format));
    }
    assert_eq!(
        UpdaterContract::decode(&vec![b' '; MAX_CONTRACT_BYTES + 1]).err(),
        Some(Rejection::Format)
    );
}
