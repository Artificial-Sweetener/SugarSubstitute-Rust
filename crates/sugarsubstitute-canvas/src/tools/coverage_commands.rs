// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Selection commands the editing UI invokes: deselect, clear, and live-previewed expand,
//! contract and feather of the selection or the whole target mask.
//!
//! A modification follows the Python app's contextual toolbar. Its first preview captures the
//! selected tool and makes Pan & Zoom effective, so the canvas can be inspected while previewing;
//! tool selection and Space only update what returns afterwards. Every preview is computed from
//! the unmodified original in an owned job, and only the latest one is staged. Apply publishes
//! the latest preview as one undo step, computing it first when it is not staged yet; Cancel
//! drops the preview. Either restores the captured tool.
use super::{
    CanvasTool, Disposition, ToolCancelReason, ToolId, ToolManager, coverage_work, deferred,
    deferred::PreparedEdit,
};
use crate::{CanvasError, Document, EditSessionId, LayerId};
use std::time::Duration;
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_image_ops::{CombineMode, Coverage};

/// The coverage a modification reshapes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModifyTarget {
    /// The document's selection.
    Selection,
    /// The whole tool target mask, as the "Edit layer coverage" panel edits it.
    Mask,
}

/// How a modification reshapes coverage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoverageOperation {
    /// Grow by a square pixel radius.
    Expand,
    /// Shrink by a square pixel radius.
    Contract,
    /// Soften edges by a Gaussian-like radius.
    Feather,
}

/// One reshaping with its radius in document pixels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoverageModification {
    operation: CoverageOperation,
    pixels: u16,
}
impl CoverageModification {
    /// Largest radius the editing UI offers.
    pub const MAX_PIXELS: u16 = 999;
    /// Validate a radius of 1 to 999 pixels.
    /// # Errors
    /// Rejects radii outside that range.
    pub fn new(operation: CoverageOperation, pixels: u32) -> Result<Self, CanvasError> {
        match u16::try_from(pixels) {
            Ok(pixels @ 1..=Self::MAX_PIXELS) => Ok(Self { operation, pixels }),
            _ => Err(CanvasError::Invalid("Coverage modification radius must be 1 to 999 pixels")),
        }
    }
    /// The reshaping.
    pub const fn operation(self) -> CoverageOperation {
        self.operation
    }
    /// Radius in document pixels.
    pub const fn pixels(self) -> u16 {
        self.pixels
    }
    fn apply(self, coverage: &Coverage, context: &JobContext) -> Result<Coverage, CanvasError> {
        let pixels = u32::from(self.pixels);
        Ok(match self.operation {
            CoverageOperation::Expand => coverage.expand(pixels, context)?,
            CoverageOperation::Contract => coverage.contract(pixels, context)?,
            CoverageOperation::Feather => coverage.feather(pixels as f32, context)?,
        })
    }
}

/// Lifecycle step of a live-previewed modification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModifyPhase {
    /// Replace the live preview, beginning the modification when none is open.
    Preview,
    /// Publish the modification as one undo step and end it.
    Apply,
    /// Drop the preview and end the modification.
    Cancel,
}

/// An unresolved modification and the request whose result it waits for.
#[derive(Debug)]
pub(super) struct Modification {
    target: ModifyTarget,
    mask: Option<LayerId>,
    latest: CoverageModification,
    request: u64,
    staged: bool,
}

impl ToolManager {
    /// The coverage an unresolved modification is reshaping; edit state reports it as the
    /// `modify` session.
    pub fn modifying(&self) -> Option<ModifyTarget> {
        self.modification.as_ref().map(|modification| modification.target)
    }
    /// Remove the selection as one undo step.
    /// # Errors
    /// Rejects deselecting during a modification and preserves document failures.
    pub fn deselect(&mut self, document: &mut Document) -> Result<bool, CanvasError> {
        self.refuse_during_modification()?;
        document.apply(crate::DocumentEdit::Select(None))
    }
    /// Remove the selected area from the target mask, as Delete does, through an owned job; the
    /// selection stays. Returns whether there was anything to clear.
    /// # Errors
    /// Rejects clearing during a modification.
    pub fn clear_selection(&mut self, document: &Document) -> Result<bool, CanvasError> {
        self.refuse_during_modification()?;
        let snapshot = document.snapshot();
        let (Some(selection), Some(mask)) = (snapshot.selection(), self.target) else {
            return Ok(false);
        };
        if coverage_work::editable_mask(&snapshot, mask).is_none() {
            return Ok(false);
        }
        let selection = selection.clone();
        let work = Box::new(move |base: &crate::DocumentSnapshot, job: &JobContext| {
            coverage_work::reshape_mask(base, mask, |coverage| {
                Ok(coverage.combine(&selection, CombineMode::Subtract, job)?)
            })
        });
        self.deferred = Some(deferred::request(
            Some(document),
            Disposition::Commit,
            &mut self.next_request,
            work,
        )?);
        Ok(true)
    }
    /// Advance a live-previewed modification of the selection or the whole target mask.
    /// Apply without an open preview publishes the modification directly.
    /// # Errors
    /// Rejects a missing selection or editable target mask, a different target while one is
    /// open, and preserves tool activation and document failures.
    pub fn modify(
        &mut self,
        target: ModifyTarget,
        modification: CoverageModification,
        phase: ModifyPhase,
        document: &mut Document,
        now: Duration,
    ) -> Result<(), CanvasError> {
        if self.modification.as_ref().is_some_and(|open| open.target != target) {
            return Err(CanvasError::Invalid("Another coverage modification is open"));
        }
        match phase {
            ModifyPhase::Cancel => self.end_modification(Some(document), now),
            ModifyPhase::Preview => {
                if self.modification.is_none() {
                    let mask = self.modification_mask(target, document)?;
                    self.switch(
                        ToolId::PAN_ZOOM,
                        Some(document),
                        ToolCancelReason::SelectionChanged,
                        now,
                    )?;
                    self.modification = Some(Modification {
                        target,
                        mask,
                        latest: modification,
                        request: 0,
                        staged: false,
                    });
                }
                self.request_modification(modification, Disposition::Preview, document)
            }
            ModifyPhase::Apply => {
                let Some(open) = &self.modification else {
                    let mask = self.modification_mask(target, document)?;
                    let work = modification_work(target, mask, modification);
                    self.deferred = Some(deferred::request(
                        Some(document),
                        Disposition::Commit,
                        &mut self.next_request,
                        work,
                    )?);
                    return Ok(());
                };
                let current = document.snapshot();
                let staged_current = self.edit.as_ref().is_some_and(|edit| {
                    let base = edit.preview();
                    let base = base.base();
                    (base.identity(), base.revision()) == (current.identity(), current.revision())
                });
                if open.staged && open.latest == modification && staged_current {
                    if let Some(edit) = self.edit.take() {
                        document.commit_edit(edit)?;
                    }
                    return self.end_modification(Some(document), now);
                }
                self.request_modification(modification, Disposition::Commit, document)
            }
        }
    }
    /// Apply finished work for the open modification; only its latest request counts.
    pub(super) fn complete_modification(
        &mut self,
        prepared: PreparedEdit,
        document: &mut Document,
        now: Duration,
    ) -> Result<bool, CanvasError> {
        let Some(open) = self.modification.as_mut() else {
            return Ok(false);
        };
        if prepared.request() != open.request {
            return Ok(false);
        }
        match prepared.disposition() {
            Disposition::Preview => {
                let next = self
                    .next_edit
                    .checked_add(1)
                    .ok_or(CanvasError::Invalid("Edit identity exhausted"))?;
                let mut session = document.begin_edit(EditSessionId::new(next)?);
                session.stage(prepared.into_edit())?;
                self.next_edit = next;
                self.edit = Some(session);
                open.staged = true;
                Ok(true)
            }
            Disposition::Commit => {
                drop(self.edit.take());
                let changed = document.apply(prepared.into_edit())?;
                self.end_modification(Some(document), now)?;
                Ok(changed)
            }
        }
    }
    /// Lifecycle loss during a modification: keep it through focus, geometry and policy
    /// changes; end it for an explicit cancel, a document change or shutdown.
    pub(super) fn interrupt_modification(
        &mut self,
        reason: ToolCancelReason,
        document: Option<&Document>,
        now: Duration,
    ) -> Result<(), CanvasError> {
        self.capture = false;
        self.navigation.input(crate::NavigationInput::Cancel, now)?;
        self.pan_zoom.cancel(reason);
        match reason {
            ToolCancelReason::Requested
            | ToolCancelReason::DocumentChanged
            | ToolCancelReason::Shutdown => {
                self.space = false;
                self.end_modification(document, now)
            }
            ToolCancelReason::SelectionChanged
            | ToolCancelReason::TemporaryNavigation
            | ToolCancelReason::FocusLost
            | ToolCancelReason::SurfaceChanged
            | ToolCancelReason::Locked => {
                self.space = false;
                Ok(())
            }
        }
    }
    fn request_modification(
        &mut self,
        modification: CoverageModification,
        disposition: Disposition,
        document: &Document,
    ) -> Result<(), CanvasError> {
        let open = self
            .modification
            .as_mut()
            .ok_or(CanvasError::Invalid("No coverage modification is open"))?;
        let work = modification_work(open.target, open.mask, modification);
        self.deferred =
            Some(deferred::request(Some(document), disposition, &mut self.next_request, work)?);
        open.latest = modification;
        open.request = self.next_request;
        open.staged = false;
        Ok(())
    }
    fn end_modification(
        &mut self,
        document: Option<&Document>,
        now: Duration,
    ) -> Result<(), CanvasError> {
        if self.modification.take().is_none() {
            return Ok(());
        }
        drop(self.edit.take());
        self.deferred = None;
        if self.space {
            return Ok(());
        }
        self.switch(self.selected, document, ToolCancelReason::SelectionChanged, now)
    }
    /// The mask a new modification of `target` reshapes, validating that it can begin.
    fn modification_mask(
        &self,
        target: ModifyTarget,
        document: &Document,
    ) -> Result<Option<LayerId>, CanvasError> {
        let snapshot = document.snapshot();
        match target {
            ModifyTarget::Selection if snapshot.selection().is_some() => Ok(None),
            ModifyTarget::Selection => Err(CanvasError::Invalid("Nothing is selected")),
            ModifyTarget::Mask => self
                .target
                .filter(|mask| coverage_work::editable_mask(&snapshot, *mask).is_some())
                .map(Some)
                .ok_or(CanvasError::Invalid("No editable mask")),
        }
    }
    fn refuse_during_modification(&self) -> Result<(), CanvasError> {
        if self.modification.is_some() {
            return Err(CanvasError::Invalid("A coverage modification is unresolved"));
        }
        Ok(())
    }
}

fn modification_work(
    target: ModifyTarget,
    mask: Option<LayerId>,
    modification: CoverageModification,
) -> deferred::Work {
    Box::new(move |base, job| match (target, mask) {
        (ModifyTarget::Selection, _) => {
            coverage_work::reshape_selection(base, |selection| modification.apply(selection, job))
        }
        (ModifyTarget::Mask, Some(mask)) => {
            coverage_work::reshape_mask(base, mask, |coverage| modification.apply(coverage, job))
        }
        (ModifyTarget::Mask, None) => Err(CanvasError::Invalid("No editable mask")),
    })
}
