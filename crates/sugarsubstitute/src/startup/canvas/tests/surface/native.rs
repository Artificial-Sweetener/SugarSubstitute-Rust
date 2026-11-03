// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route each native protocol family to its entry point and answer it with its own reply.

use super::*;

/// Decode one family's request through the public native decoder.
fn native(
    panel: &PanelSession,
    id: u64,
    kind: &str,
    command: Value,
) -> Result<CanvasNativeRequest, Box<dyn Error>> {
    Ok(CanvasNativeRequest::decode(&serde_json::to_vec(&json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":panel.as_str(),
        "requestId":id, "type":kind, "command":command
    }))?)?)
}

/// One request of every family except context, with its reply type.
fn families(
    panel: &PanelSession,
) -> Result<Vec<(CanvasNativeRequest, &'static str)>, Box<dyn Error>> {
    Ok(vec![
        (
            native(panel, 21, "source.request", json!({"type":"watch", "enabled":true}))?,
            "source.reply",
        ),
        (native(panel, 22, "geometry.request", json!({"type":"context"}))?, "geometry.reply"),
        (native(panel, 23, "edit.request", json!({"type":"state"}))?, "edit.reply"),
        (
            native(
                panel,
                24,
                "workflow.request",
                json!({"type":"sync", "workflow":"workflow-none",
                "surfaces":[]}),
            )?,
            "workflow.reply",
        ),
    ])
}

#[test]
fn every_family_answers_with_its_own_reply_and_credit_before_the_canvas_starts()
-> Result<(), Box<dyn Error>> {
    let mut canvas = super::cancellation::waiting_canvas()?;
    let identity = identity()?;
    let panel = PanelSession::for_main(identity)?;
    let memory = MemoryLease::new(64 * 1024, Box::new(Release));
    let results = [
        json!({"kind":"failure", "status":"unavailable", "message":"The canvas is not ready"}),
        json!({"kind":"context", "supported":true, "resize":false, "selectionSupported":true,
            "document":null, "selection":null}),
        json!({"kind":"failure", "status":"unsupported", "message":""}),
        json!({"kind":"failure", "status":"unavailable", "message":"The canvas is not ready"}),
    ];
    for ((request, reply_type), result) in families(&panel)?.into_iter().zip(results) {
        let id = request.request_id();
        canvas.native(identity, request, memory.claim(64 * 1024)?, Default::default())?;
        let Some(DesktopStartupCommand::SendCanvasNative(reply)) = canvas.take_command() else {
            return Err("The native request was not answered".into());
        };
        let value: Value = serde_json::from_slice(reply.payload())?;
        assert_eq!(value["type"], reply_type);
        assert_eq!(value["requestId"], id);
        assert_eq!(value["session"], panel.as_str());
        assert_eq!(value["result"], result);
        assert!(memory.claim(1).is_err(), "the reply must hold the request's credit");
        drop(reply);
        assert!(canvas.take_command().is_none());
    }
    drop(memory.claim(64 * 1024)?);
    Ok(())
}

#[test]
fn refused_families_answer_unavailable_and_a_closed_canvas_stays_silent()
-> Result<(), Box<dyn Error>> {
    let mut canvas = super::cancellation::waiting_canvas()?;
    let identity = identity()?;
    let panel = PanelSession::for_main(identity)?;
    let memory = MemoryLease::new(64 * 1024, Box::new(Release));
    for (request, reply_type) in families(&panel)? {
        canvas.refuse_native(identity, &request, memory.claim(64 * 1024)?)?;
        let Some(DesktopStartupCommand::SendCanvasNative(reply)) = canvas.take_command() else {
            return Err("The refused request was not answered".into());
        };
        let value: Value = serde_json::from_slice(reply.payload())?;
        assert_eq!(value["type"], reply_type);
        assert_eq!(value["result"]["status"], "unavailable");
    }
    canvas.surface.close(None)?;
    for (request, _) in families(&panel)? {
        canvas.native(identity, request, memory.claim(64 * 1024)?, Default::default())?;
        assert!(canvas.take_command().is_none());
    }
    drop(memory.claim(64 * 1024)?);
    Ok(())
}
