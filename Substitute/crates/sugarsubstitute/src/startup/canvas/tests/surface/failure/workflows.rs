// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Interleave two workflow tabs' attachments and selections on the one native surface.

use super::*;

const SECOND: Address<'static> = Address { workflow: "second", attachment: 2 };

#[test]
fn switching_workflows_mid_load_never_presents_the_other_workflows_pixels()
-> Result<(), Box<dyn Error>> {
    hosted(Host::new(0)?, |host| {
        commit_red(host)?;
        let before = host.committed().len();
        host.send_to(SECOND, 5, "surface.prepare", json!({}))?;
        answers(&host.expect(5, "ready")?, SECOND, 5);
        host.send_to(SECOND, 6, "surface.geometry", geometry(80, 40, false))?;
        answers(&host.expect(6, "applied")?, SECOND, 6);
        let tall = host.gate("tall.png", false);
        host.send_to(SECOND, 7, "surface.select", select("tall.png"))?;
        host.pump_until("acquisition", |_| tall.entered())?;
        // The inactive first tab's catalog update cannot reach the staged second workflow.
        host.send_to(Address::ORIGINAL, 8, "surface.select", select("green.png"))?;
        answers(&host.expect(8, "unavailable")?, Address::ORIGINAL, 8);
        tall.release();
        answers(&host.expect(7, "applied")?, SECOND, 7);
        assert_eq!(host.shown(), Some(Shade::Red));
        host.send_to(SECOND, 9, "surface.geometry", geometry(80, 40, true))?;
        answers(&host.expect(9, "applied")?, SECOND, 9);
        host.send_to(SECOND, 10, "surface.commit", json!({}))?;
        answers(&host.expect(10, "applied")?, SECOND, 10);
        host.quiesce()?;
        assert_eq!(host.shown(), Some(Shade::Blue));
        let switched = host.committed();
        assert!(switched[before..].iter().all(|shade| *shade == Shade::Blue), "{switched:?}");

        // Return to the first workflow, then leave again before its output loads.
        let returning = Address { workflow: "workflow", attachment: 3 };
        host.send_to(returning, 11, "surface.prepare", json!({}))?;
        answers(&host.expect(11, "ready")?, returning, 11);
        host.send_to(returning, 12, "surface.geometry", geometry(80, 40, false))?;
        host.expect(12, "applied")?;
        let green = host.gate("green.png", false);
        host.send_to(returning, 13, "surface.select", select("green.png"))?;
        host.pump_until("acquisition", |_| green.entered())?;
        assert_eq!(host.cancel(13)?.as_deref(), Some("cancelled"));
        host.send_to(returning, 14, "surface.release", json!({}))?;
        // The abandoned acquisition now succeeds after its selection and attachment were
        // withdrawn; restoration retains the document until that worker returns.
        green.release();
        answers(&host.expect(14, "applied")?, returning, 14);
        host.quiesce()?;
        assert_eq!(host.shown(), Some(Shade::Blue));
        let returned = host.committed();
        assert!(!returned.contains(&Shade::Green), "{returned:?}");

        host.send_to(SECOND, 15, "surface.select", select("green.png"))?;
        answers(&host.expect(15, "applied")?, SECOND, 15);
        host.quiesce()?;
        assert_eq!(host.shown(), Some(Shade::Green));
        Ok(())
    })
}

#[test]
fn old_workflow_requests_during_a_staged_switch_are_answered_stale_at_their_own_address()
-> Result<(), Box<dyn Error>> {
    hosted(Host::new(0)?, |host| {
        commit_red(host)?;
        host.send_to(SECOND, 5, "surface.prepare", json!({}))?;
        host.expect(5, "ready")?;
        let read = json!({"command":{"type":"read"}});
        host.send_to(Address::ORIGINAL, 6, "surface.viewport.request", read)?;
        answers(&host.expect(6, "stale")?, Address::ORIGINAL, 6);
        host.send_to(SECOND, 7, "surface.geometry", geometry(80, 40, false))?;
        answers(&host.expect(7, "applied")?, SECOND, 7);
        host.send_to(Address::ORIGINAL, 8, "surface.geometry", geometry(60, 30, true))?;
        answers(&host.expect(8, "stale")?, Address::ORIGINAL, 8);
        host.send_to(SECOND, 9, "surface.select", select("tall.png"))?;
        answers(&host.expect(9, "applied")?, SECOND, 9);
        host.send_to(Address::ORIGINAL, 10, "surface.commit", json!({}))?;
        answers(&host.expect(10, "stale")?, Address::ORIGINAL, 10);
        assert_eq!(host.shown(), Some(Shade::Red));
        host.send_to(SECOND, 11, "surface.geometry", geometry(80, 40, true))?;
        host.expect(11, "applied")?;
        host.send_to(SECOND, 12, "surface.commit", json!({}))?;
        answers(&host.expect(12, "applied")?, SECOND, 12);
        host.send_to(Address::ORIGINAL, 13, "surface.release", json!({}))?;
        answers(&host.expect(13, "stale")?, Address::ORIGINAL, 13);
        host.quiesce()?;
        assert_eq!(host.shown(), Some(Shade::Blue));
        Ok(())
    })
}
