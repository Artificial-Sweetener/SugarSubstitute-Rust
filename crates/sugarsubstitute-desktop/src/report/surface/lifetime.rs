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
use super::{ControlRequests, ReportCommand, ReportControl, ReportOwner, ReportPresentation};
use crate::installer::native::{ClipboardTransfer, CopyFailure, Event, Lifetime, Page, Picker};
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
        DetailsMotion, DetailsText, Interaction, ReportAction, ReportFrame, ReportKind,
        ReportLayout, ReportState, ReportStatus, WIDTH,
    },
    scene::{FramePaint, SceneResources},
    text::PaintSpec,
    widgets::Bounds,
};

mod preparation;

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
    /// Full report text revision this frame shows.
    details_revision: u64,
}

/// What a slice did, for bounded-work tests.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct SliceWork {
    pub(super) measured_layout: bool,
    pub(super) measured_lines: usize,
    pub(super) paint_steps: usize,
}

pub(crate) struct ReportLifetime<'a> {
    presentation: ReportPresentation,
    owner: Box<dyn ReportOwner + 'a>,
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
    clipboard: Option<ClipboardTransfer>,
    details_revision: u64,
    picker: Option<Picker>,
    requests: u64,
    closed: bool,
    closing: bool,
    sequence: u64,
    pub(super) last_slice: SliceWork,
}

impl<'a> ReportLifetime<'a> {
    pub(crate) fn new(
        presentation: ReportPresentation,
        owner: impl ReportOwner + 'a,
    ) -> Result<Self> {
        let details = presentation.details.as_deref().map(DetailsText::new).transpose()?;
        let inventory = ReportLayout::inventory(presentation.content.kind, &presentation.labels);
        let state = ReportState::new(presentation.content.kind);
        Ok(Self {
            presentation,
            owner: Box::new(owner),
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
            details_revision: 0,
            picker: None,
            requests: 0,
            closed: false,
            closing: false,
            sequence: 0,
            last_slice: SliceWork::default(),
        })
    }

    /// Let the owner act on the report and fold its changes into the next frame.
    fn with_control<T>(
        &mut self,
        apply: impl FnOnce(&mut dyn ReportOwner, &mut ReportControl<'_>) -> Result<T>,
    ) -> Result<T> {
        let mut message = self.presentation.content.message.clone();
        let mut changed = false;
        let mut requests = ControlRequests::default();
        let value = apply(
            &mut *self.owner,
            &mut ReportControl {
                message: &mut message,
                state: &mut self.state,
                closed: &mut self.closed,
                changed: &mut changed,
                requests: &mut requests,
            },
        )?;
        if let Some(details) = requests.details {
            self.details = Some(DetailsText::new(&details)?);
            self.presentation.details = Some(details);
            self.presentation.content.has_details = true;
            self.state.include_dump = false;
            self.details_revision += 1;
        }
        if let Some(before) = requests.copy {
            self.clipboard = self.copy_transfer(before);
        }
        if changed {
            self.presentation.content.message = message;
            // Message height and footer command widths may both have changed.
            self.layout = None;
            self.changed = true;
        }
        Ok(value)
    }

    fn copy_transfer(
        &self,
        before: Option<sugarsubstitute_contracts::execution::CounterDeadline>,
    ) -> Option<ClipboardTransfer> {
        let text = self.details.as_ref()?.text().to_owned();
        Some(ClipboardTransfer { text, before })
    }

    /// The save picker for this report and the current dump consent.
    fn export_picker(&mut self) -> Picker {
        let words = self.presentation.words;
        let (file_name, filter, extension) =
            match (self.presentation.content.kind, self.state.include_dump) {
                (ReportKind::Crash, true) => ("report.zip", words.archive_filter, "zip"),
                (ReportKind::Crash, false) => ("report.txt", words.text_filter, "txt"),
                _ => (words.export_file_name, words.text_filter, "txt"),
            };
        self.requests += 1;
        Picker {
            control: ReportAction::Export.id(),
            request: self.requests,
            kind: crate::SelectionKind::Save {
                title: words.save_title,
                file_name,
                filter,
                extension,
            },
        }
    }

    fn command(&mut self, action: ReportAction) -> Result {
        if action != ReportAction::Dismiss && !self.state.command(action).enabled {
            return Ok(());
        }
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
                if self.with_control(|owner, control| owner.copy(control))? {
                    self.clipboard = self.copy_transfer(None);
                }
            }
            ReportAction::Export => {
                let include_dump = self.state.include_dump;
                if self
                    .with_control(|owner, control| owner.export_requested(include_dump, control))?
                {
                    self.picker = Some(self.export_picker());
                }
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
                    Ok(None) => {
                        self.with_control(|owner, control| owner.export_abandoned(control))?;
                    }
                    Err(cause) => {
                        tracing::error!(operation = "report_export_selection", cause = %cause,
                            "Export destination selection failed");
                        self.state.status = Some(ReportStatus {
                            text: self.presentation.words.export_failed.into(),
                            error: true,
                        });
                        self.changed = true;
                        self.with_control(|owner, control| owner.export_abandoned(control))?;
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
        let revision = self.current.as_ref().map(|(_, geometry)| geometry.details_revision);
        revision.map_or(Ok(()), |revision| self.owner.presented(revision))
    }
    fn preferred_size(&self) -> Option<[u32; 2]> {
        self.requested
    }
    fn clipboard(&mut self) -> Option<ClipboardTransfer> {
        self.clipboard.take()
    }
    fn copied(&mut self, result: std::result::Result<(), CopyFailure>) -> Result {
        let words = self.presentation.words;
        self.state.status = Some(match &result {
            Ok(()) => ReportStatus { text: words.copied.into(), error: false },
            Err(cause) => {
                tracing::error!(operation = "report_copy", cause = ?cause, "Report copy failed");
                ReportStatus { text: words.copy_failed.into(), error: true }
            }
        });
        self.changed = true;
        self.with_control(|owner, control| owner.copied(result, control))
    }
    fn close_request(&mut self) -> Result<bool> {
        self.command(ReportAction::Dismiss)?;
        Ok(false)
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
    fn window_releasable(&mut self) -> Result<bool> {
        self.owner.window_releasable()
    }
    fn window_retired(&mut self) -> Result {
        self.owner.window_retired()
    }
    fn settled(&mut self) -> Result<bool> {
        self.owner.settled()
    }
    fn finish(&mut self) -> Result {
        self.owner.finish()
    }
}
