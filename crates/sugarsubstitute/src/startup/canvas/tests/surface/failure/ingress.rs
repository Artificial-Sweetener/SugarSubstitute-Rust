// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Refuse decoded but unauthorized surface requests without touching the shown attachment.

use super::*;

/// Require a refusal that leaves the native presentation, execution and credit untouched.
fn unchanged(host: &mut Host, committed: usize) -> Result<(), Box<dyn Error>> {
    host.quiesce()?;
    assert_eq!(host.committed().len(), committed);
    assert_eq!(host.shown(), Some(Shade::Red));
    idle(host)
}

#[test]
fn foreign_navigations_unadmitted_files_and_unplaced_geometry_change_nothing()
-> Result<(), Box<dyn Error>> {
    hosted(Host::new(0)?, |host| {
        commit_red(host)?;
        let committed = host.committed().len();
        let foreign = MainIdentity { navigation: Generation::new(9)?, ..host.fixture.identity };
        let foreign_panel = PanelSession::for_main(foreign)?;
        let message = request(&foreign_panel, 5, "surface.select", select("green.png"))?;
        let claim = host.fixture.memory.claim(64 * 1024)?;
        host.fixture.canvas.surface(foreign, message, None, claim)?;
        let Some(DesktopStartupCommand::SendSurface(reply)) = host.fixture.canvas.take_command()
        else {
            return Err("Foreign navigation lost its correlated refusal".into());
        };
        assert_eq!(reply.identity(), foreign);
        let value: Value = serde_json::from_slice(reply.payload())?;
        assert_eq!(value["status"], "stale");
        assert_eq!(value["session"], foreign_panel.as_str());
        drop(reply);
        unchanged(host, committed)?;

        let unadmitted = [
            json!({"filename":"green.gif", "subfolder":"", "type":"output"}),
            json!({"filename":"green.png", "subfolder":"", "type":"input"}),
            json!({"filename":"green.png", "subfolder":"", "type":"temp"}),
            json!({"filename":"../green.png", "subfolder":"", "type":"output"}),
        ];
        for (id, file) in (6..).zip(unadmitted) {
            host.send(1, id, "surface.select", json!({"itemId":"result:foreign", "file":file}))?;
            answers(&host.expect(id, "error")?, Address::ORIGINAL, id);
            unchanged(host, committed)?;
        }

        // Geometry is placed only against the desktop's own WebView allocation.
        let unplaced =
            request(&host.fixture.panel, 10, "surface.geometry", geometry(40, 20, true))?;
        let claim = host.fixture.memory.claim(64 * 1024)?;
        host.fixture.canvas.surface(host.fixture.identity, unplaced, None, claim)?;
        answers(&host.expect(10, "error")?, Address::ORIGINAL, 10);
        unchanged(host, committed)?;

        let foreign_attachment = Address { workflow: "workflow", attachment: 7 };
        host.send_to(foreign_attachment, 11, "surface.select", select("green.png"))?;
        answers(&host.expect(11, "stale")?, foreign_attachment, 11);
        let foreign_workflow = Address { workflow: "other", attachment: 1 };
        host.send_to(foreign_workflow, 12, "surface.select", select("green.png"))?;
        answers(&host.expect(12, "stale")?, foreign_workflow, 12);
        unchanged(host, committed)?;

        host.send(1, 13, "surface.select", select("green.png"))?;
        host.expect(13, "applied")?;
        host.quiesce()?;
        assert_eq!(host.shown(), Some(Shade::Green));
        Ok(())
    })
}
