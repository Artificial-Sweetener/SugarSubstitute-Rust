// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive one report inside the shared form window: owner turns, details motion and frame
//! preparation in bounded slices on the window's thread. Report surfaces must work when the
//! application's execution runtime has failed, so they never depend on it. Each slice does a
//! fixed amount of work and wakes the loop for the next, so paint, move and Escape stay live.
//!
//! Each frame is prepared at the report's natural size (limited to the work area) and the
//! window adopts that size when the frame is complete, so the window and its pixels change
//! together. When the window cannot take that size, the next frame is prepared at the size it
//! really has, so a prepared frame can never be left unpresented.
use super::{ReportCommand, ReportControl, ReportOwner, ReportPresentation};
use crate::installer::native::{Event, Lifetime, Page, Picker};
use std::{error::Error, sync::Arc, time::Duration};
use sugarsubstitute_contracts::{
    configuration::Theme,
    execution::{CompletionWake, Generation, MemoryLease, MemoryRelease, SurfaceId},
    presentation::{FrameIdentity, FrameShape, PreparedFrame},
};
use sugarsubstitute_ui::{
    accent::AccentPalette,
    form::{Control, Input},
    report::{
        DetailsMotion, DetailsText, Interaction, ReportAction, ReportFrame, ReportLayout,
        ReportState, ReportStatus, WIDTH,
    },
    scene::{FramePaint, SceneResources},
    text::PaintSpec,
    widgets::Bounds,
};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

/// Shared painter steps per slice; each paints at most 32,768 pixels or one glyph run.
pub(super) const PAINT_STEPS: usize = 48;
/// Details lines measured per slice.
pub(super) const MEASURE_LINES: usize = 32;
/// Space kept free around the report on its screen, as the Python report did.
const SCREEN_MARGIN: f64 = 48.0;

/// Report memory is its own; it is not admitted against an execution budget.
struct Standalone;
impl MemoryRelease for Standalone {
    fn release(&self, _: u64) {}
}

/// Peers and scroll geometry that belong to exactly one frame.
struct Geometry {
    controls: Vec<Control>,
    page_controls: Vec<Control>,
    viewport: Bounds,
    drag: Bounds,
    content_height: f32,
    scroll: f32,
}

/// What a slice did, for bounded-work tests.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct SliceWork {
    pub(super) measured_layout: bool,
    pub(super) measured_lines: usize,
    pub(super) paint_steps: usize,
}

pub(super) struct ReportLifetime<'a> {
    presentation: ReportPresentation,
    owner: &'a mut dyn ReportOwner,
    inventory: Vec<Control>,
    state: ReportState,
    theme: Theme,
    accent: AccentPalette,
    shape: Option<FrameShape>,
    requested: Option<[u32; 2]>,
    maximum_height: f32,
    resources: Option<SceneResources>,
    details: Option<DetailsText>,
    layout: Option<ReportLayout>,
    painting: Option<(FramePaint, Geometry, bool)>,
    current: Option<(PreparedFrame, Geometry)>,
    changed: bool,
    motion: Option<DetailsMotion>,
    elapsed: Duration,
    wake: Option<Arc<dyn CompletionWake>>,
    clipboard: Option<String>,
    picker: Option<Picker>,
    requests: u64,
    closed: bool,
    closing: bool,
    sequence: u64,
    pub(super) last_slice: SliceWork,
}

impl<'a> ReportLifetime<'a> {
    pub(super) fn new(
        presentation: ReportPresentation,
        owner: &'a mut dyn ReportOwner,
    ) -> Result<Self> {
        let details = presentation.details.as_deref().map(DetailsText::new).transpose()?;
        let inventory = ReportLayout::inventory(presentation.content.kind, &presentation.labels);
        let state = ReportState::new(presentation.content.kind);
        Ok(Self {
            presentation,
            owner,
            inventory,
            state,
            theme: Theme::Dark,
            accent: AccentPalette::PRODUCT,
            shape: None,
            requested: None,
            maximum_height: f32::MAX,
            resources: None,
            details,
            layout: None,
            painting: None,
            current: None,
            changed: true,
            motion: None,
            elapsed: Duration::ZERO,
            wake: None,
            clipboard: None,
            picker: None,
            requests: 0,
            closed: false,
            closing: false,
            sequence: 0,
            last_slice: SliceWork::default(),
        })
    }

    /// Let the owner act on the report and fold its changes into the next frame.
    fn with_control(
        &mut self,
        apply: impl FnOnce(&mut dyn ReportOwner, &mut ReportControl<'_>) -> Result,
    ) -> Result {
        let mut message = self.presentation.content.message.clone();
        let mut changed = false;
        apply(
            &mut *self.owner,
            &mut ReportControl {
                message: &mut message,
                state: &mut self.state,
                closed: &mut self.closed,
                changed: &mut changed,
            },
        )?;
        if changed {
            self.presentation.content.message = message;
            // Message height and footer command widths may both have changed.
            self.layout = None;
            self.changed = true;
        }
        Ok(())
    }

    fn command(&mut self, action: ReportAction) -> Result {
        if action != ReportAction::Dismiss && !self.state.command(action).enabled {
            return Ok(());
        }
        let words = self.presentation.words;
        match action {
            ReportAction::ToggleDetails => {
                self.state.expanded = !self.state.expanded;
                self.motion = Some(DetailsMotion::start(
                    self.state.expansion,
                    self.state.expanded,
                    self.elapsed,
                ));
                self.changed = true;
            }
            ReportAction::IncludeDump => {
                self.state.include_dump = !self.state.include_dump;
                self.changed = true;
            }
            ReportAction::Copy => {
                self.clipboard = self.details.as_ref().map(|details| details.text().to_owned());
            }
            ReportAction::Export => {
                self.requests += 1;
                self.picker = Some(Picker {
                    control: ReportAction::Export.id(),
                    request: self.requests,
                    kind: crate::SelectionKind::Save {
                        title: words.save_title,
                        file_name: words.export_file_name,
                        filter: words.text_filter,
                    },
                });
            }
            ReportAction::Dismiss => {
                let dismissal = ReportCommand::Action(self.presentation.content.kind.dismissal());
                self.with_control(|owner, control| owner.command(dismissal, control))?;
            }
            other => self.with_control(|owner, control| {
                owner.command(ReportCommand::Action(other), control)
            })?,
        }
        Ok(())
    }

    fn surface(&self, scale: f64, height: f32) -> Result<PaintSpec> {
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
    fn natural(&self) -> Option<Result<PaintSpec>> {
        let (layout, shape) = (self.layout.as_ref()?, self.shape?);
        let height = layout.height(self.state.expansion).min(self.maximum_height);
        Some(self.surface(shape.scale(), height))
    }

    /// Whether another slice has work to do.
    pub(super) fn pending(&self) -> bool {
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
    pub(super) fn slice(&mut self) -> Result {
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
        self.painting = Some((
            frame.into_paint(surface, self.accent, identity, lease)?,
            Geometry { controls, page_controls, viewport, drag, content_height, scroll },
            !refused,
        ));
        Ok(())
    }
}

impl Lifetime for ReportLifetime<'_> {
    fn inventory(&self) -> &[Control] {
        &self.inventory
    }
    fn picker(&mut self) -> Option<Picker> {
        self.picker.take()
    }
    fn console(&self) -> Option<crate::installer::InstallerConsole<'_>> {
        None
    }
    fn work_area(&mut self, height: f64) {
        self.maximum_height = (height - SCREEN_MARGIN).max(280.0) as f32;
        self.layout = None;
    }
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result {
        self.wake = Some(wake.clone());
        self.owner.start(wake)
    }
    fn resize(&mut self, shape: FrameShape) -> Result {
        if self.shape.map(FrameShape::scale) != Some(shape.scale()) {
            self.layout = None;
        }
        // A window that differs from the presented frame needs a frame of its own size.
        if self.current.as_ref().is_some_and(|(frame, _)| frame.shape() != shape) {
            self.changed = true;
        }
        self.shape = Some(shape);
        Ok(())
    }
    fn input(&mut self, event: Event) -> Result {
        if self.closing {
            return Ok(());
        }
        match event {
            Event::Control(id, Input::Invoke) => {
                if let Some(action) = ReportAction::from_id(id) {
                    self.command(action)?;
                }
            }
            Event::Control(..) => {}
            Event::Interaction { hovered, pressed, focused } => {
                let next = Interaction { hovered, pressed, focused };
                if next != self.state.interaction {
                    self.state.interaction = next;
                    self.changed = true;
                }
            }
            Event::Scroll(position) => {
                if !position.is_finite() {
                    return Err("Invalid report scroll position".into());
                }
                if position != self.state.scroll {
                    self.state.scroll = position.max(0.0);
                    self.changed = true;
                }
            }
            Event::Picked(request, result) => {
                if request.control != ReportAction::Export.id() || request.request != self.requests
                {
                    return Ok(());
                }
                match result {
                    Ok(Some(path)) => {
                        let command = ReportCommand::Export {
                            destination: path.into(),
                            include_dump: self.state.include_dump,
                        };
                        self.with_control(|owner, control| owner.command(command, control))?;
                    }
                    Ok(None) => {}
                    Err(cause) => {
                        tracing::error!(operation = "report_export_selection", cause = %cause,
                            "Export destination selection failed");
                        self.state.status = Some(ReportStatus {
                            text: self.presentation.words.export_failed.into(),
                            error: true,
                        });
                        self.changed = true;
                    }
                }
            }
            Event::SystemTheme(theme) => {
                if theme != self.theme {
                    self.theme = theme;
                    self.changed = true;
                }
            }
            Event::SystemAccent(accent) => {
                let accent = AccentPalette::resolve(accent);
                if accent != self.accent {
                    self.accent = accent;
                    self.changed = true;
                }
            }
        }
        Ok(())
    }
    fn advance(&mut self, elapsed: Duration) -> Result {
        self.elapsed = elapsed;
        if self.closing {
            return Ok(());
        }
        self.with_control(|owner, control| owner.poll(control))?;
        if self.motion.is_some() && self.painting.is_none() {
            self.changed = true;
        }
        self.slice()?;
        if self.pending()
            && let Some(wake) = &self.wake
        {
            wake.wake()?;
        }
        Ok(())
    }
    fn page(&self) -> Option<Page<'_>> {
        let (frame, geometry) = self.current.as_ref()?;
        Some(Page {
            frame,
            shell: &geometry.controls,
            controls: &geometry.page_controls,
            drag_bounds: geometry.drag,
            viewport: geometry.viewport,
            scroll: geometry.scroll,
            content_height: geometry.content_height,
        })
    }
    fn presented(&mut self) -> Result {
        Ok(())
    }
    fn preferred_size(&self) -> Option<[u32; 2]> {
        self.requested
    }
    fn clipboard(&mut self) -> Option<String> {
        self.clipboard.take()
    }
    fn copied(&mut self, result: std::result::Result<(), String>) -> Result {
        let words = self.presentation.words;
        self.state.status = Some(match result {
            Ok(()) => ReportStatus { text: words.copied.into(), error: false },
            Err(cause) => {
                tracing::error!(operation = "report_copy", cause = %cause, "Report copy failed");
                ReportStatus { text: words.copy_failed.into(), error: true }
            }
        });
        self.changed = true;
        Ok(())
    }
    fn close_requested(&self) -> bool {
        self.closed
    }
    fn close(&mut self) -> Result {
        self.closing = true;
        self.painting = None;
        self.motion = None;
        self.owner.close()
    }
    fn settled(&mut self) -> Result<bool> {
        self.owner.settled()
    }
    fn finish(&mut self) -> Result {
        Ok(())
    }
}
