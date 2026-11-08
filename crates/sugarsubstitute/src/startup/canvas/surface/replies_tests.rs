// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve native reply ordering and bounded admission across partial delivery.

use super::*;
use sugarsubstitute_contracts::startup::LaunchId;

/// Distinguish queued commands without constructing an unrelated canvas or native window.
fn command(sequence: u64) -> Result<DesktopStartupCommand, Box<dyn Error>> {
    Ok(DesktopStartupCommand::RetireSplash(LaunchId::new(sequence)?))
}

/// Observe the transferred command identity at the same boundary used by the UI pump.
fn delivered(surface: &mut NativeSurface) -> Result<u64, Box<dyn Error>> {
    let Some(DesktopStartupCommand::RetireSplash(launch)) = surface.take_command() else {
        return Err("Queued command was lost or replaced".into());
    };
    Ok(launch.get())
}

#[test]
fn surface_replies_keep_admission_order_when_new_replies_arrive_between_drains()
-> Result<(), Box<dyn Error>> {
    let mut surface = NativeSurface::new();
    surface.enqueue(command(1)?)?;
    surface.enqueue(command(2)?)?;
    assert_eq!(delivered(&mut surface)?, 1);
    surface.enqueue(command(3)?)?;
    assert_eq!(delivered(&mut surface)?, 2);
    assert_eq!(delivered(&mut surface)?, 3);
    assert!(surface.take_command().is_none());
    surface.enqueue(command(4)?)?;
    assert_eq!(delivered(&mut surface)?, 4);
    Ok(())
}

#[test]
fn surface_replies_reject_overload_without_displacing_older_commands() -> Result<(), Box<dyn Error>>
{
    let mut surface = NativeSurface::new();
    for sequence in 1..=8 {
        surface.enqueue(command(sequence)?)?;
    }
    assert!(surface.enqueue(command(9)?).is_err());
    assert_eq!(delivered(&mut surface)?, 1);
    surface.enqueue(command(10)?)?;
    for sequence in 2..=8 {
        assert_eq!(delivered(&mut surface)?, sequence);
    }
    assert_eq!(delivered(&mut surface)?, 10);
    assert!(surface.take_command().is_none());
    surface.enqueue(command(11)?)?;
    surface.close(None)?;
    assert!(surface.take_command().is_none());
    Ok(())
}

/// A backend withdrawal may need a terminal reply after the ordinary canvas poll succeeded.
#[test]
fn public_poll_frame_names_canvas_backend_refresh_overload() -> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::{
        execution::{Generation, MemoryLease, MemoryRelease, ScopeId},
        lifecycle::{LeaseId, ResourceGeneration, ResourceId},
        panel::PanelSession,
    };
    use sugarsubstitute_desktop::{StartupConsumeFailure, StartupConsumeStage, WindowContent};
    use sugarsubstitute_supervision::lifecycle_protocol::{BackendPeer, LeaseReference};
    struct Release;
    impl MemoryRelease for Release {
        fn release(&self, _: u64) {}
    }
    let mut host = crate::startup::platform_tests::consume::started_host()?;
    let identity = MainIdentity {
        run: host.policy.run(),
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    };
    let backend = crate::startup::CanvasBackend {
        identity,
        lease: LeaseReference {
            resource: ResourceId::new(1)?,
            generation: ResourceGeneration::new(1)?,
            lease: LeaseId::new(1)?,
        },
        origin: sugarsubstitute_contracts::configuration::LoopbackOrigin::parse(
            "http://127.0.0.1:8188",
            8188,
        )?,
        peer: BackendPeer {
            process_id: std::num::NonZeroU32::new(1).ok_or("PID absent")?,
            creation_time_100ns: std::num::NonZeroU64::new(1).ok_or("Birth absent")?,
        },
    };
    let panel = PanelSession::for_main(identity)?;
    let request = serde_json::json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":panel.as_str(),
        "requestId":1, "type":"surface.prepare", "revision":1,
        "address":{"sessionId":"canvas:test", "viewId":"view:test",
            "windowId":"window:test", "attachmentId":"attachment:test", "generation":1}
    });
    let CanvasSurfaceMessage::Request(request) =
        CanvasSurfaceMessage::decode(&serde_json::to_vec(&request)?)?
    else {
        return Err("Surface request absent".into());
    };
    let memory = MemoryLease::new(1024 * 1024, Box::new(Release));
    let mut surface = NativeSurface::new();
    surface.backend = Some(backend);
    surface.binding = Some(Binding {
        document: None,
        identity,
        backend: Some(backend),
        address: request.address().clone(),
        revision: 1,
        placement: None,
        allocation: None,
        backdrops: Vec::new(),
        backdrop_generation: 0,
        occlusions: Vec::new(),
        modal: false,
        selected: false,
        selection_revision: 0,
        source_intent_revision: None,
        presented_selection_revision: 0,
        presented_viewport: None,
        overview: false,
        presented_overview: false,
        committed: false,
        presented: None,
        presented_input: false,
        drag_name: None,
    });
    surface.pending = Some(Pending {
        identity,
        request,
        claim: memory.claim(64 * 1024)?,
        kind: PendingKind::Prepare,
        wait: crate::canvas_surface_diagnostic::Wait::None,
        preparation: None,
    });
    for sequence in 1..=8 {
        surface.enqueue(command(sequence)?)?;
    }
    let mut canvas = EmbeddedCanvas::new(
        PathBuf::new(),
        DiagnosticHealth::fixture(),
        host.clock.clone(),
        host.jobs.clone(),
        ScopeSpec { id: ScopeId::new(3)?, deadline: None },
        CanvasFunding { maximum_extent: Extent::new(64, 64)?, retained_document_bytes: 0 },
    );
    canvas.surface = surface;
    host.canvas = Some(canvas);
    let error =
        host.poll_frame(std::num::NonZeroUsize::MIN).err().ok_or("Refresh failure absent")?;
    let retained = error.downcast_ref::<StartupConsumeFailure>().ok_or("Refresh stage absent")?;
    assert_eq!(retained.stage(), StartupConsumeStage::CanvasBackendRefresh);
    let surface = &mut host.canvas.as_mut().ok_or("Canvas absent")?.surface;
    for sequence in 1..=8 {
        assert_eq!(delivered(surface)?, sequence);
    }
    crate::startup::platform_tests::consume::finish(&mut host)
}
