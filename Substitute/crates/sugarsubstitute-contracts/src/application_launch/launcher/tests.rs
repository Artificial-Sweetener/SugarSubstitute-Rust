// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A valid future launcher is not automatically compatible with this installation or app.
use super::*;
use crate::application_launch::{CURRENT_CONTRACT, INSTALLED_LAUNCH_PROTOCOL};
use serde_json::json;

#[test]
fn producing_launcher_declares_exactly_its_implemented_reader_and_entry() -> Result<(), Rejection> {
    let launcher = LauncherContract::decode(CURRENT_LAUNCHER_CONTRACT)?;
    assert_eq!(launcher.installation_schemas, [crate::installation::SCHEMA_VERSION]);
    assert_eq!(launcher.installed_launch_protocol, INSTALLED_LAUNCH_PROTOCOL);
    launcher.require_application(
        &ApplicationContract::decode(CURRENT_CONTRACT)?,
        crate::installation::SCHEMA_VERSION,
    )
}

#[test]
fn reader_and_entry_compatibility_are_independent() -> Result<(), Rejection> {
    let current = LauncherContract::decode(CURRENT_LAUNCHER_CONTRACT)?;
    let next_app =
        ApplicationContract::decode(br#"{"schema_version":1,"installed_launch_protocols":[3]}"#)?;
    assert_eq!(current.require_application(&next_app, 1), Err(Rejection::Unsupported));
    let transition_app =
        ApplicationContract::decode(br#"{"schema_version":1,"installed_launch_protocols":[1,2]}"#)?;
    current.require_application(&transition_app, 1)?;
    assert_eq!(current.require_application(&transition_app, 2), Err(Rejection::InstallationSchema),);
    let next = LauncherContract::decode(
        br#"{"schema_version":1,"installation_schemas":[1,2],"installed_launch_protocol":3}"#,
    )?;
    next.require_application(&next_app, 1)?;
    next.require_application(&next_app, 2)?;
    assert_eq!(
        next.require_application(&ApplicationContract::decode(CURRENT_CONTRACT)?, 1),
        Err(Rejection::Unsupported),
    );
    Ok(())
}

#[test]
fn trial_entry_cannot_be_used_as_the_stable_launchers_accepted_entry() -> Result<(), Rejection> {
    let application = ApplicationContract::decode(CURRENT_CONTRACT)?;
    application.require_installed_launch(crate::application_launch::TRIAL_LAUNCH_PROTOCOL)?;
    let launcher = LauncherContract::decode(
        br#"{"schema_version":1,"installation_schemas":[1],"installed_launch_protocol":2}"#,
    )?;
    assert_eq!(launcher.require_application(&application, 1), Err(Rejection::Unsupported));
    Ok(())
}

#[test]
fn malformed_launcher_declarations_cannot_claim_capabilities() {
    for versions in [
        json!([]),
        json!([0]),
        json!([1, 1]),
        json!([2, 1]),
        json!([1.5]),
        json!((1..=17).collect::<Vec<_>>()),
    ] {
        let bytes = json!({"schema_version":1,"installation_schemas":versions,"installed_launch_protocol":1}).to_string();
        assert_eq!(LauncherContract::decode(bytes.as_bytes()).err(), Some(Rejection::Format));
    }
    for bytes in [
        br#"{"schema_version":2,"installation_schemas":[1],"installed_launch_protocol":1}"#.as_slice(),
        br#"{"schema_version":1,"installation_schemas":[1],"installed_launch_protocol":0}"#,
        br#"{"schema_version":1,"installation_schemas":[1],"installed_launch_protocol":1,"command":"foreign.exe"}"#,
        br#"{"schema_version":1,"installation_schemas":[1],"installed_launch_protocol":1,"installed_launch_protocol":2}"#,
        br#"{"schema_version":1,"installed_launch_protocol":1}"#,
    ] {
        assert_eq!(LauncherContract::decode(bytes).err(), Some(Rejection::Format));
    }
    assert_eq!(
        LauncherContract::decode(&vec![b' '; MAX_CONTRACT_BYTES + 1]).err(),
        Some(Rejection::Format)
    );
}

#[test]
fn launcher_replacement_preserves_reader_and_produced_entry() -> Result<(), Rejection> {
    let previous = LauncherContract::decode(CURRENT_LAUNCHER_CONTRACT)?;
    previous.require_replacement_of(&previous, 1)?;
    for (schemas, protocol, expected) in
        [(json!([2]), 1, Rejection::InstallationSchema), (json!([1]), 2, Rejection::Unsupported)]
    {
        let declaration = json!({"schema_version":1,"installation_schemas":schemas,"installed_launch_protocol":protocol}).to_string();
        let next = LauncherContract::decode(declaration.as_bytes())?;
        assert_eq!(next.require_replacement_of(&previous, 1), Err(expected));
    }
    Ok(())
}
