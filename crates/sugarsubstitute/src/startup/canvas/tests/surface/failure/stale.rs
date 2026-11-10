// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drop work addressed to released attachments, older revisions and superseded frames.

use super::*;

#[test]
fn released_attachments_and_older_revisions_are_stale_and_start_no_work()
-> Result<(), Box<dyn Error>> {
    hosted(Host::new(0)?, |host| {
        commit_red(host)?;
        host.send(1, 5, "surface.release", json!({}))?;
        host.expect(5, "applied")?;
        host.quiesce()?;
        assert_eq!(host.shown(), None);
        let read = json!({"command":{"type":"read"}});
        let released = [
            (6, "surface.select", select("green.png")),
            (7, "surface.geometry", geometry(80, 40, true)),
            (8, "surface.viewport.request", read.clone()),
            (9, "surface.commit", json!({})),
            (10, "surface.release", json!({})),
        ];
        for (id, kind, extra) in released {
            host.send(1, id, kind, extra)?;
            answers(&host.expect(id, "stale")?, Address::ORIGINAL, id);
            idle(host)?;
        }
        let renewed = Address { workflow: "workflow", attachment: 2 };
        host.send_to(renewed, 11, "surface.prepare", json!({}))?;
        host.expect(11, "ready")?;
        host.send_to(renewed, 12, "surface.geometry", geometry(80, 40, true))?;
        host.expect(12, "applied")?;
        host.send_to(renewed, 13, "surface.select", select("tall.png"))?;
        host.expect(13, "applied")?;
        host.send_to(renewed, 14, "surface.commit", json!({}))?;
        host.expect(14, "applied")?;
        host.quiesce()?;
        assert_eq!(host.shown(), Some(Shade::Blue));
        let shown = host.committed().len();
        // Replayed and reordered page requests carry revisions at or below the attachment's.
        for id in [14, 12, 1] {
            host.send_to(renewed, id, "surface.select", select("green.png"))?;
            answers(&host.expect(id, "stale")?, renewed, id);
            idle(host)?;
        }
        host.quiesce()?;
        assert_eq!(host.committed().len(), shown);
        assert_eq!(host.shown(), Some(Shade::Blue));
        Ok(())
    })
}

#[test]
fn superseded_frames_and_foreign_hide_acknowledgements_are_never_applied()
-> Result<(), Box<dyn Error>> {
    hosted(Host::new(0)?, |host| {
        commit_red(host)?;
        let before = host.committed().len();
        select_at(host, Address::ORIGINAL, 5, "tall.png", Stage::Present, true)?;
        let stale = prepared_frame(&host.fixture.canvas)
            .map(|frame| frame.content.target())
            .ok_or("Prepared blue frame absent")?;
        assert_eq!(host.cancel(5)?.as_deref(), Some("cancelled"));
        host.send(1, 6, "surface.select", select("green.png"))?;
        host.expect(6, "applied")?;
        host.quiesce()?;
        assert_eq!(host.shown(), Some(Shade::Green));
        // A late acknowledgement of the withdrawn frame is ignored and changes nothing.
        host.fixture.canvas.presented(stale)?;
        let foreign = MainIdentity { navigation: Generation::new(2)?, ..host.fixture.identity };
        host.fixture.canvas.hidden(foreign)?;
        assert!(host.fixture.canvas.take_command().is_none());
        host.quiesce()?;
        assert_eq!(host.shown(), Some(Shade::Green));
        let committed = host.committed();
        assert!(!committed[before..].contains(&Shade::Blue), "{committed:?}");
        idle(host)
    })
}

#[test]
fn a_camera_request_for_a_retired_navigation_is_never_answered() -> Result<(), Box<dyn Error>> {
    hosted(Host::new(0)?, |host| {
        commit_red(host)?;
        let old = host.fixture.identity;
        host.send(1, 5, "surface.viewport.request", json!({"command":{"type":"zoom","factor":2}}))?;
        host.pump_until("camera motion", |fixture| prepared_frame(&fixture.canvas).is_some())?;
        host.fixture.canvas.main_retired(old)?;
        assert!(host.fixture.canvas.take_command().is_none());
        host.quiesce()?;
        assert_eq!(host.shown(), None);
        // A late hide acknowledgement for the retired page cannot resolve anything.
        host.fixture.canvas.hidden(old)?;
        assert!(host.fixture.canvas.take_command().is_none());
        idle(host)
    })
}
