// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fence a selected output when its authenticated backend grant disappears.

use super::*;
use sugarsubstitute_contracts::{
    configuration::LoopbackOrigin,
    lifecycle::{LeaseId, ResourceGeneration, ResourceId},
};
use sugarsubstitute_supervision::lifecycle_protocol::{BackendPeer, LeaseReference};

/// Inspect support through the actual surface route without acquiring an attachment.
fn discovery(
    canvas: &mut EmbeddedCanvas,
    panel: &PanelSession,
    memory: &MemoryLease,
    request_id: u64,
    expected: &str,
) -> Result<(), Box<dyn Error>> {
    canvas.surface(
        identity()?,
        discovery_request(panel, request_id)?,
        None,
        memory.claim(64 * 1024)?,
    )?;
    let Some(DesktopStartupCommand::SendSurface(reply)) = canvas.take_command() else {
        return Err("Surface discovery response absent".into());
    };
    let value: Value = serde_json::from_slice(reply.payload())?;
    assert_eq!(value["type"], "surface.capabilities.reply");
    assert_eq!(value["requestId"], request_id);
    assert_eq!(value["status"], expected);
    assert_eq!(value["capabilities"]["schema"], 1);
    assert_eq!(
        value["capabilities"]["imageFormats"],
        if expected == "available" { json!(["png"]) } else { json!([]) }
    );
    assert_eq!(value["capabilities"]["viewportCommands"], json!([]));
    assert_eq!(value["capabilities"]["previewFormats"], json!([]));
    for field in ["viewportState", "comparison", "overview"] {
        assert_eq!(value["capabilities"][field], false);
    }
    Ok(())
}

/// Admit discovery independently of when its ordinary terminal reaches the browser.
fn discovery_request(
    panel: &PanelSession,
    request_id: u64,
) -> Result<CanvasSurfaceMessage, Box<dyn Error>> {
    Ok(CanvasSurfaceMessage::decode(&serde_json::to_vec(&json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":panel.as_str(),
        "requestId":request_id, "type":"surface.capabilities.request", "schema":1
    }))?)?)
}

#[test]
fn backend_revocation_cancels_the_pending_selection_and_fences_presentation()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let clock = Arc::new(ClockValue(AtomicU64::new(0)));
    let wake = Arc::new(Wake::default());
    let mut owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let mut canvas = EmbeddedCanvas::new(
        directory.path().to_path_buf(),
        DiagnosticHealth::fixture(),
        clock,
        JobSequence::new(),
        ScopeSpec { id: ScopeId::new(1)?, deadline: None },
    );
    canvas.surface = crate::startup::canvas::surface::NativeSurface::new();
    canvas.start(client.clone(), "en")?;
    while !canvas.reported_ready {
        let revision = wake.revision();
        pump(&mut canvas, &client)?;
        if !canvas.reported_ready {
            wake.wait_after(revision)?;
        }
    }
    let identity = identity()?;
    let backend = crate::startup::CanvasBackend {
        identity,
        lease: LeaseReference {
            resource: ResourceId::new(1)?,
            generation: ResourceGeneration::new(1)?,
            lease: LeaseId::new(1)?,
        },
        origin: LoopbackOrigin::parse("http://127.0.0.1:1", 1)?,
        peer: BackendPeer {
            process_id: std::num::NonZeroU32::new(47).ok_or("PID")?,
            creation_time_100ns: std::num::NonZeroU64::new(53).ok_or("Birth")?,
        },
    };
    let panel = PanelSession::for_main(identity)?;
    let memory = MemoryLease::new(2 * 1024 * 1024, Box::new(Release));
    discovery(&mut canvas, &panel, &memory, 90, "unavailable")?;
    canvas.backend(Some(backend))?;
    discovery(&mut canvas, &panel, &memory, 91, "available")?;
    canvas.surface(
        identity,
        request(&panel, 1, "surface.prepare", json!({}))?,
        None,
        memory.claim(64 * 1024)?,
    )?;
    wait_reply(&mut canvas, &client, &wake, 1, "ready")?;
    assert!(canvas.matches_backend(backend));
    discovery(&mut canvas, &panel, &memory, 92, "available")?;
    assert!(canvas.matches_backend(backend));
    canvas.surface(
        identity,
        request(&panel, 2, "surface.select", json!({
            "itemId":"result:test", "file":{"filename":"image.png", "subfolder":"", "type":"output"}
        }))?,
        None,
        memory.claim(64 * 1024)?,
    )?;
    canvas.backend(None)?;
    reply(&mut canvas, 2, "unavailable")?;
    assert!(!canvas.matches_backend(backend));
    assert!(matches!(canvas.presentation(), MainCanvasPresentation::Hide));
    discovery(&mut canvas, &panel, &memory, 93, "unavailable")?;

    canvas.backend(Some(backend))?;
    canvas.surface(
        identity,
        request(&panel, 94, "surface.prepare", json!({}))?,
        None,
        memory.claim(64 * 1024)?,
    )?;
    wait_reply(&mut canvas, &client, &wake, 94, "ready")?;
    assert!(canvas.matches_backend(backend));
    let terminal_credit = MemoryLease::new(64 * 1024, Box::new(Release));
    canvas.surface(
        identity,
        discovery_request(&panel, 95)?,
        None,
        terminal_credit.claim(64 * 1024)?,
    )?;
    assert!(terminal_credit.claim(1).is_err());
    canvas.backend(None)?;
    assert!(!canvas.matches_backend(backend));
    let Some(DesktopStartupCommand::SendSurface(reply)) = canvas.take_command() else {
        return Err("Backend revocation discarded an admitted discovery terminal".into());
    };
    let value: Value = serde_json::from_slice(reply.payload())?;
    assert_eq!(reply.identity(), identity);
    assert_eq!(value["type"], "surface.capabilities.reply");
    assert_eq!(value["requestId"], 95);
    assert_eq!(value["session"], panel.as_str());
    assert_eq!(value["status"], "available");
    assert!(terminal_credit.claim(1).is_err());
    assert!(canvas.take_command().is_none());
    drop(reply);
    drop(terminal_credit.claim(64 * 1024)?);

    canvas.close_until(Moment::from_nanos(10_000_000_000))?;
    loop {
        let revision = wake.revision();
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        if canvas.settled()? {
            break;
        }
        wake.wait_after(revision)?;
    }
    canvas.finish()?;
    owner.request_shutdown(Moment::from_nanos(10_000_000_000));
    let deadline =
        Instant::now().checked_add(Duration::from_secs(10)).ok_or("Deadline overflow")?;
    loop {
        client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        if owner.poll_shutdown()?.settled {
            break;
        }
        if Instant::now() >= deadline {
            return Err("Execution did not settle".into());
        }
        std::hint::spin_loop();
    }
    owner.finish_shutdown()?;
    Ok(())
}
