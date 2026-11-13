// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

use super::*;
use std::error::Error;

type TestResult = Result<(), Box<dyn Error>>;

fn session() -> Result<PanelSession, PanelError> {
    PanelSession::new("session-12345678".into())
}

#[test]
fn a_description_encodes_its_protocols_and_capabilities_in_order() -> TestResult {
    let description = HostDescription::new(
        vec![HostProtocol::PANEL, HostProtocol::CANVAS, HostProtocol::MEMORY],
        vec![HostCapability::SYSTEM_ACCENT, HostCapability::HOST_OUTPUT_SURFACE],
    )?;
    let encoded: serde_json::Value = serde_json::from_slice(&description.encode(&session()?)?)?;
    assert_eq!(
        encoded,
        serde_json::json!({
            "protocol": "sugarcubes.host", "version": 1, "session": "session-12345678",
            "type": "description",
            "protocols": [
                {"name": "sugarcubes.panel", "version": 3},
                {"name": "sugarcubes.canvas", "version": 3},
                {"name": "sugarcubes.memory", "version": 1},
            ],
            "capabilities": [
                {"name": "presentation.system-accent"},
                {"name": "output.host-surface"},
            ],
        })
    );
    let decoded: wire::HostDescriptionMessage = serde_json::from_value(encoded)?;
    assert!(decoded.valid());
    assert!(description.serves(HostProtocol::CANVAS));
    assert!(!description.serves(HostProtocol::UPDATES));
    assert!(description.declares(HostCapability::HOST_OUTPUT_SURFACE));
    Ok(())
}

#[test]
fn a_description_without_capabilities_still_lists_an_empty_set() -> TestResult {
    let description = HostDescription::new(vec![HostProtocol::PANEL], Vec::new())?;
    let encoded: serde_json::Value = serde_json::from_slice(&description.encode(&session()?)?)?;
    assert_eq!(encoded["capabilities"], serde_json::json!([]));
    Ok(())
}

#[test]
fn repeated_entries_and_a_missing_panel_protocol_are_refused() {
    for (protocols, capabilities) in [
        (vec![HostProtocol::CANVAS], Vec::new()),
        (vec![HostProtocol::PANEL, HostProtocol::PANEL], Vec::new()),
        (
            vec![HostProtocol::PANEL],
            vec![HostCapability::SYSTEM_ACCENT, HostCapability::SYSTEM_ACCENT],
        ),
    ] {
        assert!(matches!(HostDescription::new(protocols, capabilities), Err(PanelError::Invalid)));
    }
}

#[test]
fn every_named_protocol_and_capability_is_a_schema_token() {
    for protocol in
        [HostProtocol::PANEL, HostProtocol::CANVAS, HostProtocol::MEMORY, HostProtocol::UPDATES]
    {
        let wire = wire::HostProtocol {
            name: protocol.name().into(),
            version: u64::from(protocol.version()),
        };
        assert!(wire.valid(), "{protocol:?}");
    }
    for capability in [HostCapability::SYSTEM_ACCENT, HostCapability::HOST_OUTPUT_SURFACE] {
        assert!(wire::HostCapability { name: capability.name().into() }.valid());
    }
}

#[test]
fn the_named_versions_and_bounds_match_the_shared_schema() -> TestResult {
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../Sugar/integrations/contracts/host.schema.json"
    ))?;
    let message = &schema["$defs"]["HostDescriptionMessage"]["properties"];
    assert_eq!(message["protocol"]["const"], PROTOCOL);
    assert_eq!(message["version"]["const"], VERSION);
    assert_eq!(message["protocols"]["maxItems"], MAX_PROTOCOLS);
    assert_eq!(message["capabilities"]["maxItems"], MAX_CAPABILITIES);
    Ok(())
}
