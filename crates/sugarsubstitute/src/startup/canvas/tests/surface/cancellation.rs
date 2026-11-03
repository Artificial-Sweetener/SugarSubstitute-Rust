// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire context reads independently of canvas initialization and backend availability.

use super::*;

/// Construct the inert application owner without starting workers or opening storage.
pub(super) fn waiting_canvas() -> Result<EmbeddedCanvas, Box<dyn Error>> {
    let mut canvas = EmbeddedCanvas::new(
        PathBuf::new(),
        DiagnosticHealth::fixture(),
        Arc::new(ClockValue(AtomicU64::new(0))),
        JobSequence::new(),
        ScopeSpec { id: ScopeId::new(1)?, deadline: None },
        funding()?,
    );
    canvas.surface = crate::startup::canvas::surface::NativeSurface::new();
    Ok(canvas)
}

/// Use the public request decoder rather than manufacturing an internal context record.
fn context(panel: &PanelSession, id: u64) -> Result<CanvasNativeRequest, Box<dyn Error>> {
    Ok(CanvasNativeRequest::decode(&serde_json::to_vec(&json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":panel.as_str(),
        "requestId":id, "type":"context.request"
    }))?)?)
}

/// Use the same cancellation envelope as the shared client's timeout and disposal paths.
fn cancel(panel: &PanelSession, id: u64) -> Result<CanvasSurfaceMessage, Box<dyn Error>> {
    Ok(CanvasSurfaceMessage::decode(&serde_json::to_vec(&json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":panel.as_str(),
        "requestId":id, "type":"cancel"
    }))?)?)
}

#[test]
fn waiting_context_cancels_before_canvas_initialization_and_retains_terminal_reply_credit()
-> Result<(), Box<dyn Error>> {
    let mut canvas = waiting_canvas()?;
    let identity = identity()?;
    let panel = PanelSession::for_main(identity)?;
    let memory = MemoryLease::new(128 * 1024, Box::new(Release));
    canvas.native(identity, context(&panel, 11)?, memory.claim(64 * 1024)?, Default::default())?;
    assert!(canvas.take_command().is_none());
    assert!(memory.claim(64 * 1024 + 1).is_err());

    canvas.surface(identity, cancel(&panel, 11)?, None, memory.claim(64 * 1024)?)?;
    let Some(DesktopStartupCommand::SendCanvasNative(reply)) = canvas.take_command() else {
        return Err("Waiting context did not produce its terminal cancellation reply".into());
    };
    let value: Value = serde_json::from_slice(reply.payload())?;
    assert_eq!(value["type"], "error.reply");
    assert_eq!(value["requestId"], 11);
    assert_eq!(value["session"], panel.as_str());
    assert_eq!(value["failure"]["status"], "cancelled");
    let Some(DesktopStartupCommand::CompletePanelIngress(receipt)) = canvas.take_command() else {
        return Err("Cancellation consumption lost its independent native receipt".into());
    };
    assert_eq!(receipt.identity(), identity);
    assert!(memory.claim(1).is_err());
    drop(reply);
    let ordinary = memory.claim(64 * 1024)?;
    drop(ordinary);
    assert!(memory.claim(64 * 1024 + 1).is_err());
    drop(receipt);
    assert!(canvas.take_command().is_none());
    let restored = memory.claim(128 * 1024)?;
    drop(restored);

    canvas.native(identity, context(&panel, 12)?, memory.claim(64 * 1024)?, Default::default())?;
    assert!(canvas.take_command().is_none());
    canvas.surface(identity, cancel(&panel, 12)?, None, memory.claim(64 * 1024)?)?;
    let Some(DesktopStartupCommand::SendCanvasNative(reply)) = canvas.take_command() else {
        return Err("The next context read did not retain its own cancellation identity".into());
    };
    let value: Value = serde_json::from_slice(reply.payload())?;
    assert_eq!(value["requestId"], 12);
    cancel_consumed(&mut canvas, identity)?;
    assert!(canvas.take_command().is_none());
    assert!(memory.claim(128 * 1024).is_err());
    drop(reply);
    drop(memory.claim(128 * 1024)?);
    Ok(())
}

#[test]
fn cancellation_cannot_retire_another_request_or_navigation() -> Result<(), Box<dyn Error>> {
    let mut canvas = waiting_canvas()?;
    let current = identity()?;
    let foreign = MainIdentity { navigation: Generation::new(2)?, ..current };
    let panel = PanelSession::for_main(current)?;
    let other = PanelSession::for_main(foreign)?;
    let memory = MemoryLease::new(128 * 1024, Box::new(Release));
    canvas.native(current, context(&panel, 11)?, memory.claim(64 * 1024)?, Default::default())?;
    for (identity, session, request_id) in [(current, &panel, 12), (foreign, &other, 11)] {
        canvas.surface(identity, cancel(session, request_id)?, None, memory.claim(64 * 1024)?)?;
        let Some(DesktopStartupCommand::CompletePanelIngress(receipt)) = canvas.take_command()
        else {
            return Err("An unrelated cancellation produced an ordinary terminal reply".into());
        };
        assert_eq!(receipt.identity(), identity);
        drop(receipt);
        assert!(canvas.take_command().is_none());
        assert!(memory.claim(64 * 1024 + 1).is_err());
    }
    canvas.surface(current, cancel(&panel, 11)?, None, memory.claim(64 * 1024)?)?;
    let Some(DesktopStartupCommand::SendCanvasNative(reply)) = canvas.take_command() else {
        return Err("The current context was lost after unrelated cancellation".into());
    };
    assert_eq!(reply.identity(), current);
    assert_eq!(serde_json::from_slice::<serde_json::Value>(reply.payload())?["requestId"], 11);
    cancel_consumed(&mut canvas, current)?;
    assert!(canvas.take_command().is_none());
    drop(reply);
    drop(memory.claim(128 * 1024)?);
    Ok(())
}

#[test]
fn navigation_retirement_removes_unattached_waits_without_discarding_newer_replies()
-> Result<(), Box<dyn Error>> {
    let mut canvas = waiting_canvas()?;
    let old = identity()?;
    let current = MainIdentity { navigation: Generation::new(2)?, ..old };
    let old_panel = PanelSession::for_main(old)?;
    let panel = PanelSession::for_main(current)?;
    let memory = MemoryLease::new(192 * 1024, Box::new(Release));
    canvas.native(old, context(&old_panel, 11)?, memory.claim(64 * 1024)?, Default::default())?;
    canvas.native(current, context(&panel, 12)?, memory.claim(64 * 1024)?, Default::default())?;
    assert!(memory.claim(64 * 1024 + 1).is_err());
    canvas.main_retired(old)?;
    let released = memory.claim(128 * 1024)?;
    drop(released);

    canvas.surface(current, cancel(&panel, 12)?, None, memory.claim(64 * 1024)?)?;
    canvas.main_retired(old)?;
    let Some(DesktopStartupCommand::SendCanvasNative(reply)) = canvas.take_command() else {
        return Err("Late old-navigation retirement discarded the current terminal reply".into());
    };
    assert_eq!(reply.identity(), current);
    assert_eq!(serde_json::from_slice::<serde_json::Value>(reply.payload())?["requestId"], 12);
    let Some(DesktopStartupCommand::CompletePanelIngress(receipt)) = canvas.take_command() else {
        return Err("Late old-navigation retirement discarded current cancellation custody".into());
    };
    assert_eq!(receipt.identity(), current);
    drop(reply);
    drop(receipt);
    assert!(canvas.take_command().is_none());
    drop(memory.claim(192 * 1024)?);
    Ok(())
}

#[test]
fn an_unstarted_canvas_refuses_surface_work_with_correlated_original_credit()
-> Result<(), Box<dyn Error>> {
    let mut canvas = waiting_canvas()?;
    let identity = identity()?;
    let panel = PanelSession::for_main(identity)?;
    let memory = MemoryLease::new(64 * 1024, Box::new(Release));
    canvas.surface(
        identity,
        request(&panel, 1, "surface.prepare", json!({}))?,
        None,
        memory.claim(64 * 1024)?,
    )?;
    let Some(DesktopStartupCommand::SendSurface(reply)) = canvas.take_command() else {
        return Err("Unavailable canvas dropped an admitted surface operation".into());
    };
    let value: Value = serde_json::from_slice(reply.payload())?;
    assert_eq!(value["requestId"], 1);
    assert_eq!(value["status"], "unavailable");
    assert!(memory.claim(1).is_err());
    drop(reply);
    drop(memory.claim(64 * 1024)?);
    Ok(())
}

#[test]
fn queued_context_terminal_survives_matching_cancel_without_a_duplicate_ordinary_reply()
-> Result<(), Box<dyn Error>> {
    let mut canvas = waiting_canvas()?;
    let identity = identity()?;
    let panel = PanelSession::for_main(identity)?;
    let memory = MemoryLease::new(128 * 1024, Box::new(Release));
    canvas.refuse_native(identity, &context(&panel, 11)?, memory.claim(64 * 1024)?)?;
    canvas.surface(identity, cancel(&panel, 11)?, None, memory.claim(64 * 1024)?)?;
    let Some(DesktopStartupCommand::SendCanvasNative(reply)) = canvas.take_command() else {
        return Err("Cancellation displaced the queued context terminal".into());
    };
    let value: Value = serde_json::from_slice(reply.payload())?;
    assert_eq!(value["type"], "context.reply");
    assert_eq!(value["requestId"], 11);
    assert_eq!(value["session"], panel.as_str());
    assert_eq!(value["context"]["canvas"]["capabilities"]["displayOutput"], false);
    assert!(memory.claim(1).is_err());
    cancel_consumed(&mut canvas, identity)?;
    assert!(canvas.take_command().is_none());
    drop(memory.claim(64 * 1024)?);
    assert!(memory.claim(128 * 1024).is_err());
    drop(reply);
    drop(memory.claim(128 * 1024)?);
    Ok(())
}

#[test]
fn discovery_without_a_session_returns_unavailable_with_its_original_terminal_credit()
-> Result<(), Box<dyn Error>> {
    let mut canvas = waiting_canvas()?;
    assert!(canvas.session.is_none());
    let identity = identity()?;
    let panel = PanelSession::for_main(identity)?;
    let message = CanvasSurfaceMessage::decode(&serde_json::to_vec(&json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":panel.as_str(),
        "requestId":17, "type":"surface.capabilities.request", "schema":1
    }))?)?;
    let memory = MemoryLease::new(64 * 1024, Box::new(Release));
    canvas.surface(identity, message, None, memory.claim(64 * 1024)?)?;
    let Some(DesktopStartupCommand::SendSurface(reply)) = canvas.take_command() else {
        return Err("Unstarted discovery dropped its ordinary reply".into());
    };
    let value: Value = serde_json::from_slice(reply.payload())?;
    assert_eq!(value["type"], "surface.capabilities.reply");
    assert_eq!(value["requestId"], 17);
    assert_eq!(value["session"], panel.as_str());
    assert_eq!(value["status"], "unavailable");
    assert_eq!(
        value["capabilities"],
        json!({
            "schema":1, "imageFormats":[], "viewportCommands":[], "viewportState":false,
            "previewFormats":[], "comparison":false, "overview":false, "input":false
        })
    );
    assert!(memory.claim(1).is_err());
    assert!(canvas.take_command().is_none());
    drop(reply);
    drop(memory.claim(64 * 1024)?);
    Ok(())
}

#[test]
fn retirement_removes_old_queued_terminals_and_receipts_and_preserves_new_fifo_credit()
-> Result<(), Box<dyn Error>> {
    let mut canvas = waiting_canvas()?;
    let old = identity()?;
    let current = MainIdentity { navigation: Generation::new(2)?, ..old };
    let old_panel = PanelSession::for_main(old)?;
    let panel = PanelSession::for_main(current)?;
    let old_credit = MemoryLease::new(128 * 1024, Box::new(Release));
    let current_credit = MemoryLease::new(128 * 1024, Box::new(Release));
    canvas.refuse_native(old, &context(&old_panel, 11)?, old_credit.claim(64 * 1024)?)?;
    canvas.refuse_native(current, &context(&panel, 12)?, current_credit.claim(64 * 1024)?)?;
    canvas.surface(old, cancel(&old_panel, 11)?, None, old_credit.claim(64 * 1024)?)?;
    canvas.surface(current, cancel(&panel, 12)?, None, current_credit.claim(64 * 1024)?)?;
    assert!(old_credit.claim(1).is_err());
    assert!(current_credit.claim(1).is_err());
    canvas.main_retired(old)?;
    drop(old_credit.claim(128 * 1024)?);
    assert!(current_credit.claim(1).is_err());
    let Some(DesktopStartupCommand::SendCanvasNative(reply)) = canvas.take_command() else {
        return Err("Retirement lost the current first ordinary terminal".into());
    };
    assert_eq!(reply.identity(), current);
    assert_eq!(serde_json::from_slice::<serde_json::Value>(reply.payload())?["requestId"], 12);
    let value: Value = serde_json::from_slice(reply.payload())?;
    assert_eq!(value["type"], "context.reply");
    canvas.main_retired(old)?;
    cancel_consumed(&mut canvas, current)?;
    assert!(canvas.take_command().is_none());
    drop(current_credit.claim(64 * 1024)?);
    assert!(current_credit.claim(128 * 1024).is_err());
    drop(reply);
    drop(current_credit.claim(128 * 1024)?);
    Ok(())
}
