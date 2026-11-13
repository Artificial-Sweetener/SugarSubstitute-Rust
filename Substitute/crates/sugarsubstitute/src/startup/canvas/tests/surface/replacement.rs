// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the acknowledged native surface through prepared and failed attachment rollback.

use super::*;
use sugarsubstitute_canvas::FrameStamp;

/// Borrow the real shared-execution fixture at its already acknowledged current frame.
pub(super) struct Replacement<'a> {
    pub(super) canvas: &'a mut EmbeddedCanvas,
    pub(super) boundary: Boundary<'a>,
    pub(super) panel: &'a PanelSession,
    pub(super) memory: &'a MemoryLease,
    pub(super) identity: MainIdentity,
    pub(super) allocation: WebViewAllocation,
}

impl Replacement<'_> {
    /// Cancel a completed selection before native acceptance, then prepare its successor.
    pub(super) fn cancel_prepared_selection(&mut self) -> Result<(), Box<dyn Error>> {
        self.select_current(400, "blue.png")?;
        let blue = self.prepared_selection()?;
        self.canvas.presented(blue)?;
        reply(self.canvas, 400, "applied")?;
        self.select_current(401, "red.png")?;
        let cancelled = self.prepared_selection()?;
        assert_ne!(cancelled, blue);
        let cancellation = CanvasSurfaceMessage::decode(&serde_json::to_vec(&json!({
            "protocol":"sugarcubes.canvas", "version":3, "session":self.panel.as_str(),
            "requestId":401, "type":"cancel"
        }))?)?;
        self.canvas.surface(self.identity, cancellation, None, self.memory.claim(64 * 1024)?)?;
        reply(self.canvas, 401, "cancelled")?;
        cancel_consumed(self.canvas, self.identity)?;
        self.canvas.presented(cancelled)?;
        self.retained(blue)?;
        self.send(400, 402, "surface.prepare", json!({}))?;
        self.boundary.wait_reply(self.canvas, 402, "ready", "request 402 surface.prepare")?;
        self.retained(blue)?;
        Ok(())
    }

    /// Select through the committed address instead of a staged candidate.
    fn select_current(&mut self, id: u64, filename: &str) -> Result<(), Box<dyn Error>> {
        self.boundary.clock.0.fetch_add(1_000_000_000, Ordering::SeqCst);
        self.canvas.surface(
            self.identity,
            request(
                self.panel,
                id,
                "surface.select",
                json!({
                    "itemId":format!("result:{filename}"),
                    "file":{"filename":filename,"subfolder":"","type":"output"}
                }),
            )?,
            None,
            self.memory.claim(64 * 1024)?,
        )
    }

    /// Hold the real current CPU frame without supplying its native acknowledgement.
    fn prepared_selection(&mut self) -> Result<FrameStamp, Box<dyn Error>> {
        self.boundary.until(self.canvas, "the prepared frame of the current selection", |canvas| {
            if let Some(frame) = prepared_frame(canvas) {
                assert!(frame.content.pending());
                let target = frame.content.target();
                assert!(canvas.take_command().is_none());
                return Ok(Some(target));
            }
            if canvas.take_command().is_some() {
                return Err("Selection completed before its native frame was acknowledged".into());
            }
            Ok(None)
        })
    }

    /// Prepare both actual pixels and a failing original, then restore the same committed image.
    pub(super) fn rollback(&mut self, current: FrameStamp) -> Result<(), Box<dyn Error>> {
        for (base, filename, status) in [(100, "red.png", "applied"), (200, "missing.png", "error")]
        {
            self.send(base, base, "surface.prepare", json!({}))?;
            self.boundary.wait_reply(
                self.canvas,
                base,
                "ready",
                &format!("request {base} surface.prepare to attachment:{base}"),
            )?;
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
            self.boundary.clock.0.fetch_add(1_000_000_000, Ordering::SeqCst);
            self.send(base, base + 2, "surface.select", json!({
                "itemId":"result:candidate", "file":{"filename":filename,"subfolder":"","type":"output"}
            }))?;
            self.boundary.wait_reply(
                self.canvas,
                base + 2,
                status,
                &format!("request {} surface.select of {filename} to attachment:{base}", base + 2),
            )?;
            self.retained(current)?;
            self.canvas.presented(current)?;
            self.send(base, base + 3, "surface.release", json!({}))?;
            self.boundary.wait_reply(
                self.canvas,
                base + 3,
                "applied",
                &format!("request {} surface.release of attachment:{base}", base + 3),
            )?;
            let frame = prepared_frame(self.canvas).ok_or("Restored native frame is absent")?;
            assert_eq!(frame.content.target(), current);
            assert!(!frame.content.pending());
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
