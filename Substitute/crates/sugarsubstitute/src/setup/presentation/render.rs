// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain installer peer geometry alongside its shared, clipped native pixel preparation.
use super::*;
use sugarsubstitute_contracts::presentation::{FrameIdentity, PreparedFrame};
use sugarsubstitute_ui::scene::{FramePaint, Layer};

/// One admitted installer preparation. The execution adapter drives it outside the native thread.
pub struct Painting {
    paint: FramePaint,
    layout: Layout,
}

/// Pixels and native peer facts for exactly the same immutable application revision.
pub struct Prepared {
    /// Validated premultiplied surface pixels with their original admission custody.
    pub frame: PreparedFrame,
    /// Native controls, viewport and console placement corresponding to these exact pixels.
    pub layout: Layout,
}

impl Scene {
    /// Transfer scene commands to the shared painter while retaining native geometry admission.
    /// Reserve `METADATA_BYTES + FramePaint::reservation_bytes(shape)` before composing this scene;
    /// native presentation copies are accounted separately by the desktop adapter.
    /// # Errors
    /// Preserves invalid geometry, command and aggregate memory-admission failures.
    pub fn into_paint(self, identity: FrameIdentity, lease: MemoryLease) -> io::Result<Painting> {
        let s = self.surface;
        let paint = FramePaint::new(
            s,
            self.accent,
            identity,
            vec![
                Layer {
                    viewport: Bounds {
                        x: 0.0,
                        y: 0.0,
                        width: s.width as f32 / s.scale,
                        height: s.height as f32 / s.scale,
                    },
                    elements: self.shell,
                },
                Layer {
                    viewport: Bounds { width: self.layout.page_width, ..self.layout.viewport },
                    elements: self.page,
                },
            ],
            lease,
        )?;
        Ok(Painting { paint, layout: self.layout })
    }
}

impl Painting {
    /// Advance the same bounded shared renderer used by native surfaces, never on the UI thread.
    /// # Errors
    /// Preserves actual rendering and memory causes; incomplete results cannot be published.
    pub fn step(&mut self, resources: &mut SceneResources) -> io::Result<bool> {
        self.paint.step(resources)
    }

    /// Publish validated pixels and their original geometry together without rescanning either.
    /// # Errors
    /// Rejects publication before the shared renderer reaches its terminal state.
    pub fn finish(self) -> io::Result<Prepared> {
        Ok(Prepared { frame: self.paint.finish()?, layout: self.layout })
    }
}
