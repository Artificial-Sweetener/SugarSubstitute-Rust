// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the acknowledged native surface through prepared and failed attachment rollback.

use super::*;
use sugarsubstitute_canvas::FrameStamp;

/// Borrow the real shared-execution fixture at its already acknowledged current frame.
pub(super) struct Replacement<'a> {
    pub(super) canvas: &'a mut EmbeddedCanvas,
    pub(super) client: &'a ExecutionClient,
    pub(super) wake: &'a Wake,
    pub(super) clock: &'a ClockValue,
    pub(super) panel: &'a PanelSession,
    pub(super) memory: &'a MemoryLease,
    pub(super) identity: MainIdentity,
    pub(super) allocation: WebViewAllocation,
}

impl Replacement<'_> {
    /// Prepare both actual pixels and a failing original, then restore the same committed image.
    pub(super) fn rollback(&mut self, current: FrameStamp) -> Result<(), Box<dyn Error>> {
        for (base, filename, status) in [(100, "red.png", "applied"), (200, "missing.png", "error")]
        {
            self.send(base, base, "surface.prepare", json!({}))?;
            wait_reply(self.canvas, self.client, self.wake, base, "ready")?;
            self.retained(current)?;
            self.send(
                base,
                base + 1,
                "surface.geometry",
                json!({"geometry":{
                    "coordinateSpace":"client", "units":"css-pixels", "origin":{"x":10,"y":20},
                    "viewport":{"width":8,"height":8}, "clip":{"x":10,"y":20,"width":0,"height":0},
                    "physicalPixelsPerCssPixel":1, "visible":false, "focused":false
                }}),
            )?;
            reply(self.canvas, base + 1, "applied")?;
            self.clock.0.fetch_add(1_000_000_000, Ordering::SeqCst);
            self.send(base, base + 2, "surface.select", json!({
                "itemId":"result:candidate", "file":{"filename":filename,"subfolder":"","type":"output"}
            }))?;
            wait_reply(self.canvas, self.client, self.wake, base + 2, status)?;
            self.retained(current)?;
            assert!(self.canvas.presented(current).is_err());
            self.send(base, base + 3, "surface.release", json!({}))?;
            wait_reply(self.canvas, self.client, self.wake, base + 3, "applied")?;
            let frame = prepared_frame(self.canvas).ok_or("Restored native frame is absent")?;
            assert_eq!(frame.frame.target, current);
            assert!(!frame.frame.pending);
            self.send(base, base + 4, "surface.commit", json!({}))?;
            reply(self.canvas, base + 4, "stale")?;
        }
        Ok(())
    }

    /// Send a distinct exact-address candidate through the same decoded public contract.
    fn send(
        &mut self,
        attachment: u64,
        id: u64,
        operation: &str,
        mut extra: Value,
    ) -> Result<(), Box<dyn Error>> {
        extra.as_object_mut().ok_or("Request fields absent")?.insert(
            "address".into(),
            json!({
                "sessionId":"canvas:workflow", "viewId":"view:one", "windowId":"window:one",
                "attachmentId":format!("attachment:{attachment}"), "generation":attachment
            }),
        );
        self.canvas.surface(
            self.identity,
            request(self.panel, id, operation, extra)?,
            Some(self.allocation),
            self.memory.claim(64 * 1024)?,
        )
    }

    /// Observe retained native pixels independently of the candidate's prepared CPU frame.
    fn retained(&self, target: FrameStamp) -> Result<(), Box<dyn Error>> {
        let MainCanvasPresentation::Retain(retained) = self.canvas.presentation() else {
            return Err("Candidate replaced the preceding native pixels before commit".into());
        };
        assert_eq!(retained.target, target);
        Ok(())
    }
}
