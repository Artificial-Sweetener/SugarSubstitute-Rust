// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject unauthorized wire shapes before obtaining the prepared native download grant.
use super::*;
use serde_json::json;

/// Unknown operation fields, noncanonical chunks and excessive extents never reach native effects.
#[test]
fn download_operations_reject_unbounded_or_ambiguous_input() {
    for operation in [
        json!({"op":"entry","key":"","root":"E:/foreign"}),
        json!({"op":"entry","key":"😀".repeat(513)}),
        json!({"op":"read","name":"payload","root":"E:/foreign"}),
        json!({"op":"open","name":"payload","expected":25_u64*1024*1024*1024}),
        json!({"op":"chunk","name":"payload","offset":0,"hex":"AB"}),
        json!({"op":"chunk","name":"payload","offset":0,"hex":"a"}),
        json!({"op":"chunk","name":"payload","offset":0,"hex":""}),
        json!({"op":"chunk","name":"payload","offset":0,"hex":"00".repeat(24*1024+1)}),
        json!({"op":"write","name":"payload","hex":"00".repeat(8193)}),
        json!({"op":"reset","name":"x".repeat(4097)}),
    ] {
        let request = json!({"version":1,"serial":1,"operation":operation});
        assert!(
            serde_json::from_value::<Request>(request)
                .map_or(true, |request| request.validate().is_err())
        );
    }
}

/// Preserve supported empty sidecars, Unicode identities and the maximum binary chunk.
#[test]
fn native_download_wire_accepts_current_bounded_shapes()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    for operation in [
        json!({"op":"entry","key":"糖😀/upstream"}),
        json!({"op":"write","name":"sidecar","hex":""}),
        json!({"op":"chunk","name":"entry/payload","offset":42,"hex":"00".repeat(24*1024)}),
        json!({"op":"open","name":"entry/payload","expected":null}),
        json!({"op":"close","name":"entry/payload"}),
    ] {
        let request: Request =
            serde_json::from_value(json!({"version":1,"serial":1,"operation":operation}))?;
        request.validate()?;
    }
    let envelope: Envelope = serde_json::from_value(
        json!({"version":1,"event":"downloads","id":"command-1","sequence":12,"data":{"version":1,"serial":1,"operation":{"op":"validate"}}}),
    )?;
    assert_eq!(envelope.sequence, 12);
    assert_eq!(envelope.id, "command-1");
    Ok(())
}
