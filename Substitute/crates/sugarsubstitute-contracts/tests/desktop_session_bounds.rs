// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject every shared schema bound before portable semantic admission.
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::error::Error;
use sugarsubstitute_contracts::desktop_session::wire::*;

#[derive(Deserialize)]
struct Probe {
    field: String,
    kind: String,
    value: Value,
    item: Option<Value>,
    valid: bool,
}
#[derive(Deserialize)]
struct Shape {
    name: String,
    base: Value,
    probes: Vec<Probe>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Vectors {
    schema_sha256: String,
    shapes: Vec<Shape>,
}

/// Dispatch through the generated decoder and its exact schema admission, without domain policy.
fn valid(name: &str, value: &Value) -> bool {
    macro_rules! admit {
        ($kind:ty) => {
            serde_json::from_value::<$kind>(value.clone()).is_ok_and(|value| value.valid())
        };
    }
    match name {
        "SessionScope" => admit!(SessionScope),
        "SessionSource" => admit!(SessionSource),
        "SessionScene" => admit!(SessionScene),
        "SessionFile" => admit!(SessionFile),
        "SessionAsset" => admit!(SessionAsset),
        "SessionOutput" => admit!(SessionOutput),
        "SessionSelection" => admit!(SessionSelection),
        "SessionCamera" => admit!(SessionCamera),
        "SessionInspection" => admit!(SessionInspection),
        "SessionComparison" => admit!(SessionComparison),
        "SessionView" => admit!(SessionView),
        "SessionGraphView" => admit!(SessionGraphView),
        "SessionDocument" => admit!(SessionDocument),
        "SessionInput" => admit!(SessionInput),
        "SessionSubmission" => admit!(SessionSubmission),
        "SessionEntry" => admit!(SessionEntry),
        "SessionRect" => admit!(SessionRect),
        "SessionPlacement" => admit!(SessionPlacement),
        "SessionCheckpoint" => admit!(SessionCheckpoint),
        "SessionBinding" => admit!(SessionBinding),
        "SessionFence" => admit!(SessionFence),
        "SessionCapabilities" => admit!(SessionCapabilities),
        "SessionTransfer" => admit!(SessionTransfer),
        "SessionFailure" => admit!(SessionFailure),
        "SessionOperation" => admit!(SessionOperation),
        "SessionMessage" => admit!(SessionMessage),
        "SessionNavigation" => admit!(SessionNavigation),
        _ => false,
    }
}

#[test]
fn shared_shape_vectors_reject_every_field_bound_unknown_member_and_missing_member()
-> Result<(), Box<dyn Error>> {
    let vectors: Vectors = serde_json::from_str(include_str!(
        "../../../../Sugar/integrations/contracts/session_shape_vectors.json"
    ))?;
    let schema = include_str!("../../../../Sugar/integrations/contracts/session.schema.json")
        .replace("\r\n", "\n");
    assert_eq!(vectors.schema_sha256, format!("{:x}", Sha256::digest(schema.as_bytes())));
    let shapes = vectors.shapes;
    assert_eq!(shapes.len(), 37);
    assert_eq!(shapes.iter().map(|shape| shape.probes.len()).sum::<usize>(), 730);
    for shape in shapes {
        assert!(valid(&shape.name, &shape.base), "{} baseline", shape.name);
        for probe in shape.probes {
            let mut value = shape.base.clone();
            let object = value.as_object_mut().ok_or("Expected shape object")?;
            match probe.kind.as_str() {
                "missing" => {
                    object.remove(&probe.field);
                }
                "string" => {
                    object.insert(
                        probe.field.clone(),
                        Value::String("x".repeat(usize::try_from(
                            probe.value.as_u64().ok_or("Expected count")?,
                        )?)),
                    );
                }
                "array" => {
                    object.insert(
                        probe.field.clone(),
                        Value::Array(vec![
                            probe.item.ok_or("Expected item")?;
                            usize::try_from(
                                probe.value.as_u64().ok_or("Expected count")?
                            )?
                        ]),
                    );
                }
                _ => {
                    object.insert(probe.field.clone(), probe.value);
                }
            }
            assert_eq!(
                valid(&shape.name, &value),
                probe.valid,
                "{} {} {}",
                shape.name,
                probe.field,
                probe.kind
            );
        }
    }
    Ok(())
}
