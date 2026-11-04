// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Advertise host presentation only for a composition-hosted browser and bind the bulk path.

use super::*;
use crate::main_bulk::Endpoint;
use std::error::Error;

type TestResult = Result<(), Box<dyn Error>>;

fn local() -> Result<Endpoint, DesktopError> {
    Endpoint::for_root("http://127.0.0.1:8188/")
}

#[test]
fn composition_hosting_invokes_the_protocol_with_host_presentation() -> TestResult {
    let [protocol, bulk, transport] = scripts(MainHosting::Composition, &local()?)?;
    assert!(protocol.starts_with(PROTOCOL_SCRIPT.trim_end()));
    assert!(protocol.ends_with("})('host');\n"));
    assert_eq!(bulk, format!("{}(\"/__sugarsubstitute/bulk/\");\n", BULK_SCRIPT.trim_end()));
    assert_eq!(transport, TRANSPORT_SCRIPT);
    Ok(())
}

#[test]
fn windowed_hosting_leaves_the_output_surface_preference_absent() -> TestResult {
    let [protocol, bulk, transport] = scripts(MainHosting::Windowed, &local()?)?;
    assert!(protocol.starts_with(PROTOCOL_SCRIPT.trim_end()));
    assert!(protocol.ends_with("})(undefined);\n"));
    assert!(!protocol.ends_with("('host');\n"));
    assert_eq!(bulk, format!("{}(\"/__sugarsubstitute/bulk/\");\n", BULK_SCRIPT.trim_end()));
    assert_eq!(transport, TRANSPORT_SCRIPT);
    Ok(())
}

#[test]
fn the_bulk_script_receives_the_base_path_as_one_string_literal() -> TestResult {
    let endpoint = Endpoint::for_root("https://proxy.example/it's%22/")?;
    let [_, bulk, _] = scripts(MainHosting::Windowed, &endpoint)?;
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
        for hosting in [MainHosting::Composition, MainHosting::Windowed] {
            for source in scripts(hosting, &endpoint)? {
                assert!(
                    serde_json::to_vec(&serde_json::json!({ "source": source }))?.len() <= 8192
                );
            }
        }
    }
    Ok(())
}
