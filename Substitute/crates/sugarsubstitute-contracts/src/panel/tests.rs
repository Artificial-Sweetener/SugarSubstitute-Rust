// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify Rust and SugarCubes against identical closed panel wire vectors.

use super::*;
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct Vector {
    label: String,
    valid: bool,
    value: serde_json::Value,
}

#[test]
fn generated_panel_decoder_identifies_the_retained_schema() -> Result<(), Box<dyn Error>> {
    let schema = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../third_party/sugarcubes-panel/panel.schema.json"
    ))
    .replace("\r\n", "\n");
    let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/panel/wire.rs"));
    let recorded = source
        .lines()
        .find_map(|line| line.strip_prefix("// Schema SHA-256 (LF): "))
        .ok_or("Generated panel schema fingerprint absent")?;
    assert_eq!(recorded, format!("{:x}", Sha256::digest(schema.as_bytes())));
    Ok(())
}

#[test]
fn shared_panel_vectors_match_schema_and_roundtrip_without_losing_fields()
-> Result<(), Box<dyn Error>> {
    let vectors: Vec<Vector> = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../third_party/sugarcubes-panel/panel_vectors.json"
    )))?;
    assert!(!vectors.is_empty());
    for vector in vectors {
        let encoded = serde_json::to_vec(&vector.value)?;
        let decoded = PanelMessage::decode(&encoded);
        assert_eq!(decoded.is_ok(), vector.valid, "{}", vector.label);
        if let Ok(message) = decoded {
            let emitted = message.encode()?;
            assert_eq!(serde_json::from_slice::<serde_json::Value>(&emitted)?, vector.value);
            assert_eq!(PanelMessage::decode(&emitted)?, message);
        }
    }
    Ok(())
}

#[test]
fn rejects_duplicate_members_and_oversized_inputs_before_authorization() {
    for bytes in [
        br#"{"protocol":"sugarcubes.panel","version":3,"session":"session-12345678","type":"ready","type":"ready"}"#.as_slice(),
        br#"{"protocol":"sugarcubes.panel","version":3,"session":"session-12345678","session":"session-12345678","type":"ready"}"#.as_slice(),
    ] {
        assert!(PanelMessage::decode(bytes).is_err());
    }
    assert!(matches!(
        PanelMessage::decode(&vec![b' '; MAX_PANEL_MESSAGE_BYTES + 1]),
        Err(PanelError::TooLarge)
    ));
}

#[test]
fn constructors_validate_session_presentation_and_all_supported_styles()
-> Result<(), Box<dyn Error>> {
    assert!(PanelSession::new("short".into()).is_err());
    assert!(PanelSession::new("session-12345678\n".into()).is_err());
    let session = PanelSession::new("session-12345678".into())?;
    for style in [
        PanelStyle::Comfy,
        PanelStyle::Fluent,
        PanelStyle::Spectrum,
        PanelStyle::PhotoPro,
        PanelStyle::Gimp,
    ] {
        for palette in [
            PanelPalette::Black,
            PanelPalette::DarkGray,
            PanelPalette::MediumGray,
            PanelPalette::LightGray,
        ] {
            let presentation =
                PanelPresentation::new(false, style, palette, Some("#aF0123".into()), 64)?;
            let value = PanelMessage::Configure { session: session.clone(), presentation };
            assert_eq!(PanelMessage::decode(&value.encode()?)?, value);
        }
    }
    assert!(
        PanelPresentation::new(
            false,
            PanelStyle::Comfy,
            PanelPalette::Black,
            Some("#GG0000".into()),
            0
        )
        .is_err()
    );
    assert!(
        PanelPresentation::new(false, PanelStyle::Comfy, PanelPalette::Black, None, 65).is_err()
    );
    Ok(())
}

/// The operating-system accent reaches SugarCubes through the existing presentation member.
#[test]
fn system_accent_roundtrips_as_the_presentation_accent_color() -> Result<(), Box<dyn Error>> {
    let accent = crate::appearance::AccentColor::new(0, 120, 212);
    let session = PanelSession::new("session-12345678".into())?;
    let presentation = PanelPresentation::new(
        false,
        PanelStyle::Comfy,
        PanelPalette::DarkGray,
        Some(accent.css_hex()),
        0,
    )?;
    let value = PanelMessage::Configure { session, presentation };
    let encoded = value.encode()?;
    let json: serde_json::Value = serde_json::from_slice(&encoded)?;
    assert_eq!(json["presentation"]["accentColor"], "#0078d4");
    let PanelMessage::Configure { presentation, .. } = PanelMessage::decode(&encoded)? else {
        return Err("Configure message changed kind".into());
    };
    assert_eq!(presentation.accent_color(), Some("#0078d4"));
    Ok(())
}

#[test]
fn main_session_distinguishes_complete_run_and_navigation_identity() -> Result<(), Box<dyn Error>> {
    use crate::{
        execution::Generation,
        lifecycle::{
            CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey,
        },
        startup::{LaunchId, MainIdentity},
    };
    let base = MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        },
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    };
    let original = PanelSession::for_main(base)?;
    assert_eq!(PanelSession::for_main(base)?, original);
    let mut changed = base;
    changed.run.owner.namespace = NamespaceId::new([3; 32]);
    assert_ne!(PanelSession::for_main(changed)?, original);
    changed = base;
    changed.run.owner.epoch = CoordinatorEpoch::new([3; 16])?;
    assert_ne!(PanelSession::for_main(changed)?, original);
    changed = base;
    changed.run.instance = InstanceId::new(2)?;
    assert_ne!(PanelSession::for_main(changed)?, original);
    changed = base;
    changed.run.generation = RunGeneration::new(2)?;
    assert_ne!(PanelSession::for_main(changed)?, original);
    changed = base;
    changed.launch = LaunchId::new(2)?;
    assert_ne!(PanelSession::for_main(changed)?, original);
    changed = base;
    changed.navigation = Generation::new(2)?;
    assert_ne!(PanelSession::for_main(changed)?, original);
    Ok(())
}

#[test]
fn session_constructor_does_not_retain_oversized_caller_capacity() -> Result<(), Box<dyn Error>> {
    let mut text = String::with_capacity(8192);
    text.push_str("session-12345678");
    let session = PanelSession::new(text)?;
    assert_eq!(session.0.capacity(), session.0.len());
    Ok(())
}
