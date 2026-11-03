// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire an old backend grant or page navigation mid-selection and serve its successor.

use super::*;
use sugarsubstitute_contracts::{
    configuration::LoopbackOrigin,
    lifecycle::{LeaseId, ResourceGeneration, ResourceId},
};
use sugarsubstitute_supervision::lifecycle_protocol::{BackendPeer, LeaseReference};

/// One authenticated backend grant for `identity`; a reconnect advances `generation`.
fn grant(
    identity: MainIdentity,
    generation: u64,
) -> Result<crate::startup::CanvasBackend, Box<dyn Error>> {
    Ok(crate::startup::CanvasBackend {
        identity,
        lease: LeaseReference {
            resource: ResourceId::new(1)?,
            generation: ResourceGeneration::new(generation)?,
            lease: LeaseId::new(generation)?,
        },
        origin: LoopbackOrigin::parse("http://127.0.0.1:1", 1)?,
        peer: BackendPeer {
            process_id: std::num::NonZeroU32::new(47).ok_or("PID")?,
            creation_time_100ns: std::num::NonZeroU64::new(53).ok_or("Birth")?,
        },
    })
}

/// Admit the next page's context, require its own reply, and adopt it as the fixture's page.
fn navigate(host: &mut Host, navigation: u64) -> Result<MainIdentity, Box<dyn Error>> {
    let previous = host.fixture.identity;
    let identity = MainIdentity { navigation: Generation::new(navigation)?, ..previous };
    let panel = PanelSession::for_main(identity)?;
    let claim = host.fixture.memory.claim(64 * 1024)?;
    host.fixture.canvas.native(identity, context(&panel, 1)?, claim, Default::default())?;
    let Some(DesktopStartupCommand::SendCanvasNative(reply)) = host.fixture.canvas.take_command()
    else {
        return Err("The new page's context reply is missing".into());
    };
    assert_eq!(reply.identity(), identity);
    let value: Value = serde_json::from_slice(reply.payload())?;
    assert_eq!(value["context"]["canvas"]["capabilities"]["displayOutput"], true);
    drop(reply);
    assert!(host.fixture.canvas.take_command().is_none());
    host.fixture.identity = identity;
    host.fixture.panel = panel;
    Ok(identity)
}

/// Prepare, reveal, select and commit `filename` on the current page's first attachment.
fn present(host: &mut Host, filename: &str) -> Result<(), Box<dyn Error>> {
    host.send(1, 1, "surface.prepare", json!({}))?;
    host.expect(1, "ready")?;
    host.send(1, 2, "surface.geometry", geometry(80, 40, true))?;
    host.expect(2, "applied")?;
    host.send(1, 3, "surface.select", select(filename))?;
    host.expect(3, "applied")?;
    host.send(1, 4, "surface.commit", json!({}))?;
    host.expect(4, "applied")?;
    host.quiesce()
}

#[test]
fn backend_revocation_mid_selection_retires_the_attachment_and_a_new_grant_serves_new_work()
-> Result<(), Box<dyn Error>> {
    hosted(Host::new(0)?, |host| {
        commit_red(host)?;
        let identity = host.fixture.identity;
        host.fixture.canvas.backend(Some(grant(identity, 1)?))?;
        let staged = Address { workflow: "workflow", attachment: 2 };
        host.send_to(staged, 5, "surface.prepare", json!({}))?;
        host.expect(5, "ready")?;
        host.send_to(staged, 6, "surface.geometry", geometry(80, 40, false))?;
        host.expect(6, "applied")?;
        let tall = host.gate("tall.png", true);
        host.send_to(staged, 7, "surface.select", select("tall.png"))?;
        host.pump_until("acquisition", |_| tall.entered())?;
        host.fixture.canvas.backend(None)?;
        answers(&host.expect(7, "unavailable")?, staged, 7);
        host.quiesce()?;
        assert!(tall.cancelled());
        assert_eq!(tall.retirements(), 1);
        assert_eq!(host.shown(), None);
        idle(host)?;
        host.send_to(staged, 8, "surface.geometry", geometry(80, 40, true))?;
        host.expect(8, "stale")?;

        host.fixture.canvas.backend(Some(grant(identity, 2)?))?;
        let renewed = Address { workflow: "workflow", attachment: 3 };
        host.send_to(renewed, 9, "surface.prepare", json!({}))?;
        host.expect(9, "ready")?;
        host.send_to(renewed, 10, "surface.geometry", geometry(80, 40, true))?;
        host.expect(10, "applied")?;
        host.send_to(renewed, 11, "surface.select", select("green.png"))?;
        host.expect(11, "applied")?;
        host.send_to(renewed, 12, "surface.commit", json!({}))?;
        host.expect(12, "applied")?;
        host.quiesce()?;
        assert_eq!(host.shown(), Some(Shade::Green));
        idle(host)
    })
}

#[test]
fn page_reload_mid_selection_retires_the_old_navigation_and_serves_the_new_page()
-> Result<(), Box<dyn Error>> {
    hosted(Host::new(0)?, |host| {
        commit_red(host)?;
        let old = host.fixture.identity;
        let old_panel = host.fixture.panel.clone();
        let tall = host.gate("tall.png", true);
        host.send(1, 5, "surface.select", select("tall.png"))?;
        host.pump_until("acquisition", |_| tall.entered())?;
        // The reloaded page's first context request retires the old page's attachment.
        navigate(host, 2)?;
        host.fixture.canvas.main_retired(old)?;
        host.quiesce()?;
        assert!(tall.cancelled());
        assert_eq!(tall.retirements(), 1);
        assert_eq!(host.shown(), None);
        idle(host)?;
        // Traffic still in flight from the old page is answered only to the retired navigation,
        // whose late retirement discards it before delivery.
        let late = request(&old_panel, 6, "surface.select", select("green.png"))?;
        let claim = host.fixture.memory.claim(64 * 1024)?;
        host.fixture.canvas.surface(old, late, None, claim)?;
        host.fixture.canvas.main_retired(old)?;
        assert!(host.fixture.canvas.take_command().is_none());
        present(host, "green.png")?;
        assert_eq!(host.shown(), Some(Shade::Green));
        idle(host)
    })
}

#[test]
fn navigating_away_and_back_presents_only_the_returning_page() -> Result<(), Box<dyn Error>> {
    hosted(Host::new(0)?, |host| {
        commit_red(host)?;
        let before = host.committed().len();
        host.fixture.canvas.main_retired(host.fixture.identity)?;
        host.quiesce()?;
        assert_eq!(host.shown(), None);
        idle(host)?;
        navigate(host, 3)?;
        present(host, "tall.png")?;
        assert_eq!(host.shown(), Some(Shade::Blue));
        let committed = host.committed();
        assert!(committed[before..].iter().all(|shade| *shade == Shade::Blue), "{committed:?}");
        idle(host)
    })
}
