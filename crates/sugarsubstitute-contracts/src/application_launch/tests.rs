// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Declared compatibility remains explicit through historical and future protocol versions.
use super::*;
use serde_json::json;

#[test]
fn packaged_contract_supports_the_actual_entry_protocol() -> Result<(), Rejection> {
    let application = ApplicationContract::decode(CURRENT_CONTRACT)?;
    assert_eq!(
        application.installed_launch_protocols,
        [INSTALLED_LAUNCH_PROTOCOL, TRIAL_LAUNCH_PROTOCOL]
    );
    assert_eq!(
        INSTALLED_LAUNCH_ARGUMENT,
        format!("--installed-launch-v{INSTALLED_LAUNCH_PROTOCOL}")
    );
    assert_eq!(TRIAL_LAUNCH_ARGUMENT, format!("--installed-launch-v{TRIAL_LAUNCH_PROTOCOL}"));
    Ok(())
}

#[test]
fn future_protocols_are_not_confused_with_compatible_entry_points() -> Result<(), Rejection> {
    let future =
        ApplicationContract::decode(br#"{"schema_version":1,"installed_launch_protocols":[2]}"#)?;
    assert_eq!(future.require_installed_launch(1), Err(Rejection::Unsupported));
    let transition =
        ApplicationContract::decode(br#"{"schema_version":1,"installed_launch_protocols":[1,2]}"#)?;
    transition.require_installed_launch(1)?;
    transition.require_installed_launch(2)?;
    assert_eq!(transition.require_installed_launch(0), Err(Rejection::Unsupported));
    Ok(())
}

#[test]
fn unknown_or_ambiguous_contracts_fail_closed() {
    for protocols in
        [json!([]), json!([0]), json!([1, 1]), json!([2, 1]), json!([1.5]), json!(vec![1; 17])]
    {
        let bytes = json!({"schema_version":1,"installed_launch_protocols":protocols}).to_string();
        assert!(ApplicationContract::decode(bytes.as_bytes()).is_err());
    }
    for bytes in [
        br#"{"schema_version":2,"installed_launch_protocols":[1]}"#.as_slice(),
        br#"{"schema_version":1,"installed_launch_protocols":[1],"command":"other.exe"}"#,
        br#"{"schema_version":1,"schema_version":1,"installed_launch_protocols":[1]}"#,
        br#"{"schema_version":1}"#,
    ] {
        assert!(ApplicationContract::decode(bytes).is_err());
    }
    assert!(ApplicationContract::decode(&vec![b' '; MAX_CONTRACT_BYTES + 1]).is_err());
}
