// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare report frames in bounded slices on the window's thread: measure the layout, then the
//! details text behind the first frame, then paint at most [`PAINT_STEPS`] steps per slice.
use super::*;

impl ReportLifetime<'_> {
    pub(super) fn surface(&self, scale: f64, height: f32) -> Result<PaintSpec> {
        let shape = FrameShape::new(
            (f64::from(WIDTH) * scale).round() as u32,
            (f64::from(height) * scale).ceil() as u32,
            scale,
        )?;
        Ok(PaintSpec {
            theme: self.theme,
            scale: scale as f32,
            width: shape.width(),
            height: shape.height(),
        })
    }

    /// Physical size the report needs at its current expansion, limited to the work area; a
    /// shorter window scrolls the report body between its title bar and footer.
    pub(super) fn natural(&self) -> Option<Result<PaintSpec>> {
        let (layout, shape) = (self.layout.as_ref()?, self.shape?);
        let height = layout.height(self.state.expansion).min(self.maximum_height);
        Some(self.surface(shape.scale(), height))
    }

    /// Whether another slice has work to do.
    pub(in crate::report::surface) fn pending(&self) -> bool {
        !self.closing
            && self.shape.is_some()
            && (self.painting.is_some()
                || self.motion.is_some()
                || self.changed
                || self.layout.is_none()
                || self.details.as_ref().is_some_and(|details| {
                    self.shape.is_some_and(|shape| {
                        self.surface(shape.scale(), 1.0).is_ok_and(|spec| {
                            !details.measured(ReportLayout::details_text_width(), spec)
                        })
                    })
                }))
    }

    /// One bounded slice: one layout measurement, or up to [`MEASURE_LINES`] details lines
    /// and [`PAINT_STEPS`] painter steps.
    pub(in crate::report::surface) fn slice(&mut self) -> Result {
        self.last_slice = SliceWork::default();
        let Some(shape) = self.shape.filter(|_| !self.closing) else { return Ok(()) };
        let scale = shape.scale();
        let measure_spec = self.surface(scale, 1.0)?;
        if self.resources.is_none() {
            self.resources = Some(SceneResources::new(self.presentation.locale)?);
        }
        let resources = self.resources.as_mut().ok_or("Report fonts absent")?;
        if self.layout.is_none() {
            self.layout = Some(ReportLayout::measure(
                &self.presentation.content,
                &self.presentation.labels,
                &self.state,
                measure_spec,
                resources,
                None,
                self.maximum_height,
            )?);
            self.last_slice.measured_layout = true;
            self.changed = true;
            return Ok(());
        }
        // The first frame comes first; the details text measures behind it.
        if self.current.is_some()
            && let Some(details) = &mut self.details
        {
            let width = ReportLayout::details_text_width();
            if !details.measured(width, measure_spec) {
                let (lines, height) = (details.line_count_measured(), details.height());
                let done = details.measure(width, measure_spec, resources, None, MEASURE_LINES)?;
                self.last_slice.measured_lines = details.line_count_measured() - lines;
                // Repaint an open panel once its visible band is filled, then once at the end.
                let band = self.state.scroll + 300.0;
                let filled = height < band && details.height() >= band;
                if self.state.expansion > 0.0 && (filled || done) {
                    self.changed = true;
                }
            }
        }
        if let Some((paint, _, _)) = &mut self.painting {
            let mut finished = false;
            while self.last_slice.paint_steps < PAINT_STEPS {
                self.last_slice.paint_steps += 1;
                if paint.step(resources)? {
                    finished = true;
                    break;
                }
            }
            if finished {
                let (paint, geometry, natural) =
                    self.painting.take().ok_or("Report painting absent")?;
                let frame = paint.finish()?;
                if natural {
                    self.requested = Some([frame.shape().width(), frame.shape().height()]);
                }
                self.current = Some((frame, geometry));
            }
            return Ok(());
        }
        if !self.changed {
            return Ok(());
        }
        self.changed = false;
        if let Some(motion) = self.motion {
            let (expansion, done) = motion.sample(self.elapsed);
            self.state.expansion = expansion;
            if done {
                self.motion = None;
            } else {
                // Keep sampling the motion with the next frame.
                self.changed = true;
            }
        }
        let natural = self.natural().ok_or("Report layout absent")??;
        let natural_size = [natural.width, natural.height];
        // The window already took this natural size once and kept a different client size.
        let refused =
            self.requested == Some(natural_size) && [shape.width(), shape.height()] != natural_size;
        let target = if refused { [shape.width(), shape.height()] } else { natural_size };
        let layout = self.layout.as_ref().ok_or("Report layout absent")?;
        let surface = PaintSpec {
            theme: self.theme,
            scale: scale as f32,
            width: target[0],
            height: target[1],
        };
        let ReportFrame {
            shell,
            page,
            controls,
            page_controls,
            viewport,
            drag,
            content_height,
            scroll,
        } = layout.compose(
            &self.presentation.content,
            &self.presentation.labels,
            &self.state,
            surface,
            self.details.as_ref(),
        )?;
        self.state.scroll = scroll;
        self.sequence += 1;
        let identity = FrameIdentity {
            surface: SurfaceId::new(1)?,
            state_generation: Generation::new(1)?,
            sequence: Generation::new(self.sequence)?,
        };
        let shape = FrameShape::new(target[0], target[1], scale)?;
        let lease = MemoryLease::new(
            FramePaint::reservation_bytes(shape)
                + crate::installer::native::storage_bytes(shape, 0)?,
            Box::new(Standalone),
        );
        let frame = ReportFrame {
            shell,
            page,
            controls: Vec::new(),
            page_controls: Vec::new(),
            viewport,
            drag,
            content_height,
            scroll,
        };
        let details_revision = self.details_revision;
        self.painting = Some((
            frame.into_paint(surface, self.accent, identity, lease)?,
            Geometry {
                controls,
                page_controls,
                viewport,
                drag,
                content_height,
                scroll,
                details_revision,
            },
            !refused,
        ));
        Ok(())
    }
}
