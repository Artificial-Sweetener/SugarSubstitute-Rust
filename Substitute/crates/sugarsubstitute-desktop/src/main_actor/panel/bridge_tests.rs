// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Install the transport policy unchanged and bind the bulk path.

use super::*;
use crate::main_bulk::Endpoint;
use std::error::Error;

type TestResult = Result<(), Box<dyn Error>>;

fn local() -> Result<Endpoint, DesktopError> {
    Endpoint::for_root("http://127.0.0.1:8188/")
}

#[test]
fn the_policy_installs_unchanged_and_the_bulk_script_receives_its_path() -> TestResult {
    let [protocol, bulk, transport] = scripts(&local()?)?;
    assert_eq!(protocol, PROTOCOL_SCRIPT);
    assert!(protocol.trim_end().ends_with("})();"));
    assert_eq!(bulk, format!("{}(\"/__sugarsubstitute/bulk/\");\n", BULK_SCRIPT.trim_end()));
    assert_eq!(transport, TRANSPORT_SCRIPT);
    Ok(())
}

#[test]
fn host_facts_never_travel_on_the_injected_bridge() {
    for property in ["preferencePolicy", "outputSurfacePreference", "memoryBudget"] {
        assert!(!PROTOCOL_SCRIPT.contains(property), "{property}");
        assert!(!TRANSPORT_SCRIPT.contains(property), "{property}");
    }
}

#[test]
fn the_bulk_script_receives_the_base_path_as_one_string_literal() -> TestResult {
    let endpoint = Endpoint::for_root("https://proxy.example/it's%22/")?;
    let [_, bulk, _] = scripts(&endpoint)?;
    let literal = serde_json::to_string(endpoint.path())?;
    assert_eq!(bulk, format!("{}({literal});\n", BULK_SCRIPT.trim_end()));
    assert_eq!(serde_json::from_str::<String>(&literal)?, "/it's%22/__sugarsubstitute/bulk/");
    assert!(!BULK_SCRIPT.contains(".invalid"));
    Ok(())
}

#[test]
fn composed_scripts_fit_the_native_registration_command() -> TestResult {
    let longest = format!("https://proxy.example/{}/", "x".repeat(400));
    for endpoint in [local()?, Endpoint::for_root(&longest)?] {
        for source in scripts(&endpoint)? {
            assert!(serde_json::to_vec(&serde_json::json!({ "source": source }))?.len() <= 8192);
        }
    }
    Ok(())
}
