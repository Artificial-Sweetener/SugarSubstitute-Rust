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
