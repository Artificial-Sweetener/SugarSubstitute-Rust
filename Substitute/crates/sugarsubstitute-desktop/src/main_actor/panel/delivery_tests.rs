// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise final command admission using shared protocol replies and escaped data.

use super::*;
use serde_json::{Value, json};
use std::error::Error;
use sugarsubstitute_contracts::canvas_surface::{CanvasSurfaceMessage, CanvasSurfaceReplyStatus};

/// Build the largest escaped address components allowed by the shared schema.
fn address() -> Value {
    json!({
        "sessionId": format!("canvas:{}", "\\".repeat(249)),
        "viewId": format!("view:{}", "\\".repeat(123)),
        "windowId": format!("window:{}", "\\".repeat(121)),
        "attachmentId": format!("attachment:{}", "\\".repeat(117)),
        "generation": 9007199254740991_u64,
    })
}

#[test]
fn escaped_shared_reply_fits_the_actual_native_command_without_changing_its_data()
-> Result<(), Box<dyn Error>> {
    let request = json!({
        "protocol":"sugarcubes.canvas", "version":3,
        "session":"session-12345678", "requestId":2147483647,
        "type":"surface.prepare", "address":address(), "revision":9007199254740991_u64,
    });
    let CanvasSurfaceMessage::Request(request) =
        CanvasSurfaceMessage::decode(&serde_json::to_vec(&request)?)?
    else {
        return Err("Expected surface request".into());
    };
    for message in [
        "\\".repeat(512),
        "\0".repeat(512),
        "🙂".repeat(512),
        "\"});throw Error('injected');//</script>\u{2028}\u{2029}".into(),
    ] {
        let payload = request.reply(CanvasSurfaceReplyStatus::Error, &message)?;
        let parameters = encode(i64::MAX, &payload)?;
        assert!(parameters.len() <= MAX_PARAMETERS_BYTES);
        let parameters: Value = serde_json::from_str(&parameters)?;
        assert_eq!(parameters["contextId"], i64::MAX);
        assert_eq!(parameters["returnByValue"], true);
        let expression = parameters["expression"].as_str().ok_or("Missing expression")?;
        let argument = expression
            .strip_prefix(PREFIX)
            .and_then(|value| value.strip_suffix(')'))
            .ok_or("Missing delivery argument")?;
        assert_eq!(
            serde_json::from_str::<Value>(argument)?,
            serde_json::from_slice::<Value>(&payload)?
        );
        assert_eq!(serde_json::from_str::<Value>(argument)?["message"], message);
        let ticket = crate::main_panel_ingress::Ticket(9007199254740991);
        let owned = encode_owned(i64::MAX, &payload, ticket)?;
        assert!(owned.len() <= MAX_PARAMETERS_BYTES);
        let owned: Value = serde_json::from_str(&owned)?;
        let expression = owned["expression"].as_str().ok_or("Missing owned expression")?;
        let suffix = format!(",{})", ticket.0);
        let argument = expression
            .strip_prefix(PREFIX)
            .and_then(|value| value.strip_suffix(suffix.as_str()))
            .ok_or("Missing exact private ticket")?;
        assert_eq!(
            serde_json::from_str::<Value>(argument)?,
            serde_json::from_slice::<Value>(&payload)?
        );
    }
    Ok(())
}

#[test]
fn final_parameters_admit_the_exact_byte_boundary_and_refuse_the_next_byte()
-> Result<(), Box<dyn Error>> {
    let empty = serde_json::to_vec(&json!({"message":""}))?;
    let overhead = encode(17, &empty)?.len();
    let exact =
        serde_json::to_vec(&json!({"message":"x".repeat(MAX_PARAMETERS_BYTES - overhead)}))?;
    assert_eq!(encode(17, &exact)?.len(), MAX_PARAMETERS_BYTES);
    let excess =
        serde_json::to_vec(&json!({"message":"x".repeat(MAX_PARAMETERS_BYTES - overhead + 1)}))?;
    assert!(encode(17, &excess).is_err());
    assert!(encode(17, b"{\"x\":\"\xff\"}").is_err());
    assert!(encode(17, &vec![b' '; MAX_PAYLOAD_BYTES + 1]).is_err());
    Ok(())
}

#[test]
fn ordered_delivery_hands_the_second_message_over_only_after_the_first_is_accepted()
-> Result<(), Box<dyn Error>> {
    let first = serde_json::to_vec(&json!({"type":"description","text":"\"),x("}))?;
    let second = serde_json::to_vec(&json!({"type":"configure"}))?;
    let parameters: Value = serde_json::from_str(&encode_ordered(17, &first, &second)?)?;
    assert_eq!(parameters["contextId"], 17);
    let expression = parameters["expression"].as_str().ok_or("Missing expression")?;
    let (head, tail) = expression.split_once(&format!(")&&{PREFIX}")).ok_or("Missing order")?;
    let head = head.strip_prefix(PREFIX).ok_or("Missing first delivery")?;
    let tail = tail.strip_suffix(')').ok_or("Missing second delivery")?;
    assert_eq!(serde_json::from_str::<Value>(head)?, serde_json::from_slice::<Value>(&first)?);
    assert_eq!(serde_json::from_str::<Value>(tail)?, serde_json::from_slice::<Value>(&second)?);
    let oversized = serde_json::to_vec(&json!({"text":"x".repeat(MAX_PARAMETERS_BYTES)}))?;
    assert!(encode_ordered(17, &first, &oversized).is_err());
    assert!(encode_ordered(17, b"{\"x\":\"\xff\"}", &second).is_err());
    Ok(())
}

#[test]
fn full_presentation_snapshots_fit_the_actual_no_ticket_command_at_shared_field_bounds()
-> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::{
        canvas_surface::{
            CanvasSurfaceAddress, CanvasSurfacePresentationChange,
            CanvasSurfacePresentationDisplay, CanvasSurfacePresentationStatus, CanvasViewportMode,
            CanvasViewportState,
        },
        panel::PanelSession,
    };
    for character in ["\\", "\u{1f642}"] {
        let address = CanvasSurfaceAddress::decode(&serde_json::to_vec(&json!({
            "sessionId": format!("canvas:{}", character.repeat(249)),
            "viewId": format!("view:{}", character.repeat(123)),
            "windowId": format!("window:{}", character.repeat(121)),
            "attachmentId": format!("attachment:{}", character.repeat(117)),
            "generation": 9007199254740991_u64,
        }))?)?;
        for message in ["\0".repeat(512), "\u{1f642}".repeat(512), "\\".repeat(512)] {
            let token = "t".repeat(24);
            let viewport = CanvasViewportState::new(CanvasViewportMode::Fit, 1.0, 8192, 8192)?;
            let change = CanvasSurfacePresentationChange::new(
                address.clone(),
                9007199254740991,
                token.clone(),
                9007199254740991,
                CanvasSurfacePresentationStatus::Failed,
                CanvasSurfacePresentationDisplay::Preview {
                    token,
                    lease_revision: 9007199254740991,
                    viewport,
                },
                message,
            )?;
            let payload = change.encode(&PanelSession::new("s".repeat(80))?)?;
            let parameters = encode(i64::MAX, &payload)?;
            assert!(parameters.len() <= MAX_PARAMETERS_BYTES);
            let parameters: Value = serde_json::from_str(&parameters)?;
            let expression = parameters["expression"].as_str().ok_or("Missing expression")?;
            let argument = expression
                .strip_prefix(PREFIX)
                .and_then(|value| value.strip_suffix(')'))
                .ok_or("Missing no-ticket notification")?;
            let actual: Value = serde_json::from_str(argument)?;
            assert_eq!(actual, serde_json::from_slice::<Value>(&payload)?);
            assert_eq!(actual["requestId"], 0);
            assert_eq!(actual["type"], "surface.presentation.changed");
        }
    }
    Ok(())
}
