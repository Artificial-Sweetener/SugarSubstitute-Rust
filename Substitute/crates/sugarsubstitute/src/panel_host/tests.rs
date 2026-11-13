// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove what the host declares to each browser and the presentation it configures.

use super::*;
use std::error::Error;
use sugarsubstitute_contracts::panel::{PanelMessage, PanelSession};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

fn session() -> Result<PanelSession> {
    Ok(PanelSession::new("panel-0123456789abcdef".into())?)
}

#[test]
fn every_browser_is_told_the_protocols_served_and_the_system_accent() -> Result {
    for hosting in [MainHosting::Composition, MainHosting::Windowed] {
        let description = description(hosting)?;
        for protocol in [HostProtocol::PANEL, HostProtocol::CANVAS, HostProtocol::MEMORY] {
            assert!(description.serves(protocol), "{hosting:?} {protocol:?}");
        }
        assert!(description.declares(HostCapability::SYSTEM_ACCENT), "{hosting:?}");
    }
    Ok(())
}

#[test]
fn only_a_composition_hosted_browser_is_told_the_host_presents_output() -> Result {
    assert!(description(MainHosting::Composition)?.declares(HostCapability::HOST_OUTPUT_SURFACE));
    assert!(!description(MainHosting::Windowed)?.declares(HostCapability::HOST_OUTPUT_SURFACE));
    Ok(())
}

#[test]
fn the_description_names_no_host_styles_and_encodes_as_the_shared_schema_requires() -> Result {
    let encoded = description(MainHosting::Composition)?.encode(&session()?)?;
    let value: serde_json::Value = serde_json::from_slice(&encoded)?;
    assert_eq!(
        value,
        serde_json::json!({
            "protocol": "sugarcubes.host", "version": 1, "session": "panel-0123456789abcdef",
            "type": "description",
            "protocols": [
                { "name": "sugarcubes.panel", "version": 3 },
                { "name": "sugarcubes.canvas", "version": 3 },
                { "name": "sugarcubes.memory", "version": 1 },
            ],
            "capabilities": [{ "name": "presentation.system-accent" }, { "name": "output.host-surface" }],
        })
    );
    Ok(())
}

#[test]
fn the_presentation_carries_the_system_accent_or_null_and_no_photopro_palette() -> Result {
    let session = session()?;
    for (accent, expected) in [
        (Some(AccentColor::new(0, 120, 212)), serde_json::json!("#0078d4")),
        (None, serde_json::Value::Null),
    ] {
        let configure = PanelMessage::Configure {
            session: session.clone(),
            presentation: presentation(accent)?,
        };
        let value: serde_json::Value = serde_json::from_slice(&configure.encode()?)?;
        let presentation = value.get("presentation").ok_or("presentation missing")?;
        assert_eq!(presentation.get("accentColor"), Some(&expected));
        assert_eq!(presentation.get("visualStyle"), Some(&serde_json::json!("comfy")));
        assert!(presentation.get("photoProPalette").is_none());
    }
    Ok(())
}
