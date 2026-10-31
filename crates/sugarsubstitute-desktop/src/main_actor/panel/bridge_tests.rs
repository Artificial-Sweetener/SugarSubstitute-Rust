// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Advertise host presentation to the page only for a composition-hosted browser.

use super::*;

#[test]
fn composition_hosting_invokes_the_protocol_with_host_presentation() {
    let [protocol, transport] = scripts(MainHosting::Composition);
    assert!(protocol.starts_with(PROTOCOL_SCRIPT.trim_end()));
    assert!(protocol.ends_with("})('host');\n"));
    assert_eq!(transport, TRANSPORT_SCRIPT);
}

#[test]
fn windowed_hosting_leaves_the_output_surface_preference_absent() {
    let [protocol, transport] = scripts(MainHosting::Windowed);
    assert!(protocol.starts_with(PROTOCOL_SCRIPT.trim_end()));
    assert!(protocol.ends_with("})(undefined);\n"));
    assert!(!protocol.ends_with("('host');\n"));
    assert_eq!(transport, TRANSPORT_SCRIPT);
}

#[test]
fn composed_scripts_fit_the_native_registration_command() -> Result<(), serde_json::Error> {
    for hosting in [MainHosting::Composition, MainHosting::Windowed] {
        for source in scripts(hosting) {
            assert!(serde_json::to_vec(&serde_json::json!({ "source": source }))?.len() <= 8192);
        }
    }
    Ok(())
}
