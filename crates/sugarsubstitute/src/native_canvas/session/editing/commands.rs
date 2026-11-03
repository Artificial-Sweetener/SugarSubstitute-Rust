// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Carry out `edit.request` commands on the shown input document.
use super::{registry::tool_id, *};
use std::fmt;
use sugarsubstitute_canvas::{CanvasError, CoverageOperation, ModifyPhase, ToolCancelReason};
use sugarsubstitute_contracts::canvas_native::{
    CanvasBrushOptions, CanvasBrushTool, CanvasCoverageModify, CanvasEditCommand, CanvasEditMode,
    CanvasHistoryAction, CanvasModifyOperation, CanvasModifyPhase, CanvasModifyTarget,
    CanvasSelectionAction,
};

/// An edit command the shown document cannot carry out; nothing changed.
#[derive(Debug)]
pub(crate) enum EditRejection {
    /// No editable input document is shown, or the command has nothing to act on now.
    Unavailable(&'static str),
    /// Move, Transform, edit sessions and nudging are not implemented by this host.
    Unsupported,
}

impl fmt::Display for EditRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(message) => formatter.write_str(message),
            Self::Unsupported => formatter.write_str("The canvas does not implement this command"),
        }
    }
}

impl Error for EditRejection {}

/// Report a tool's refusal of an inapplicable command as a rejection, keeping other causes.
fn refused(cause: CanvasError) -> Box<dyn Error> {
    match cause {
        CanvasError::Invalid(message) => Box::new(EditRejection::Unavailable(message)),
        cause => Box::new(cause),
    }
}

/// Brush options from the editing UI's whole-percent values.
pub(super) fn brush_options(options: CanvasBrushOptions) -> Result<BrushOptions, CanvasError> {
    BrushOptions::new(
        u32::from(options.size()),
        f64::from(options.hardness()) / 100.0,
        f64::from(options.opacity()) / 100.0,
    )
}

impl Session {
    /// Carry out one editing command on the shown input document. Reading the state changes
    /// nothing and always succeeds.
    ///
    /// # Errors
    /// Rejects commands without an editable input document, unavailable tools and commands a
    /// tool cannot apply now with [`EditRejection`], changing nothing; preserves document,
    /// tool and job admission failures.
    pub(crate) fn edit(&mut self, command: CanvasEditCommand) -> Result<(), Box<dyn Error>> {
        if command == CanvasEditCommand::State {
            return Ok(());
        }
        if matches!(
            command,
            CanvasEditCommand::Nudge { .. }
                | CanvasEditCommand::Transform(_)
                | CanvasEditCommand::Session(_)
        ) {
            return Err(Box::new(EditRejection::Unsupported));
        }
        if self.closing || self.editable_document().is_none() {
            return Err(Box::new(EditRejection::Unavailable("No editable input document")));
        }
        self.register_editing_tools()?;
        let now = self.navigation_time();
        match command {
            CanvasEditCommand::Tool(tool) => self.choose_tool(tool, now)?,
            CanvasEditCommand::Options { tool, options } => {
                let tool = match tool {
                    CanvasBrushTool::Brush => CanvasToolId::Brush,
                    CanvasBrushTool::Eraser => CanvasToolId::Eraser,
                };
                self.set_brush(tool, brush_options(options)?)?;
            }
            CanvasEditCommand::History(action) => self.step_history(action, now)?,
            CanvasEditCommand::Commit => self.commit_modification(now)?,
            CanvasEditCommand::Cancel => self.with_document(|tools, document| {
                tools.cancel(ToolCancelReason::Requested, Some(document), now)
            })?,
            CanvasEditCommand::Selection(CanvasSelectionAction::Deselect) => {
                self.with_document(|tools, document| tools.deselect(document).map(drop))?;
            }
            CanvasEditCommand::Selection(CanvasSelectionAction::Clear) => {
                self.with_document(|tools, document| tools.clear_selection(document).map(drop))?;
            }
            CanvasEditCommand::Modify(modify) => self.modify_coverage(modify, now)?,
            CanvasEditCommand::Mode(mode) => {
                self.editing.coverage_mode = mode == CanvasEditMode::Coverage;
                if self.editing.coverage_mode && self.document.tools.captured() {
                    self.with_document(|tools, document| {
                        tools.cancel(ToolCancelReason::FocusLost, Some(document), now)
                    })?;
                }
            }
            CanvasEditCommand::Navigation { held } => self.with_document(|tools, document| {
                tools.temporary_navigation(held, Some(document), now)
            })?,
            CanvasEditCommand::State
            | CanvasEditCommand::Nudge { .. }
            | CanvasEditCommand::Transform(_)
            | CanvasEditCommand::Session(_) => {}
        }
        if self.document.tools.modifying().is_none() {
            self.editing.modification = None;
        }
        self.settle_tools()?;
        self.apply_editing(now)
    }

    /// Run one tool-manager operation against the shown document's published content.
    fn with_document(
        &mut self,
        operation: impl FnOnce(
            &mut sugarsubstitute_canvas::ToolManager,
            &mut sugarsubstitute_canvas::Document,
        ) -> Result<(), CanvasError>,
    ) -> Result<(), Box<dyn Error>> {
        let document = &mut *self.document;
        let scene = document.scene.as_mut().ok_or("The input document vanished")?;
        operation(&mut document.tools, &mut scene.document).map_err(refused)
    }

    fn choose_tool(&mut self, tool: CanvasToolId, now: Duration) -> Result<(), Box<dyn Error>> {
        let Some(id) = tool_id(tool) else {
            return Err(Box::new(EditRejection::Unsupported));
        };
        if !self.tool_available(tool) {
            return Err(Box::new(EditRejection::Unavailable("The tool is unavailable")));
        }
        self.with_document(|tools, document| tools.select(id, Some(document), now))?;
        self.editing.tool = tool;
        Ok(())
    }

    /// Undo or redo one document step, first cancelling a gesture in progress; an unresolved
    /// modification must be applied or cancelled first.
    fn step_history(
        &mut self,
        action: CanvasHistoryAction,
        now: Duration,
    ) -> Result<(), Box<dyn Error>> {
        if self.document.tools.modifying().is_some() {
            return Err(Box::new(EditRejection::Unavailable("A coverage modification is open")));
        }
        self.with_document(|tools, document| {
            if tools.captured() || tools.preview().is_some() {
                tools.cancel(ToolCancelReason::Requested, Some(document), now)?;
            }
            match action {
                CanvasHistoryAction::Undo => document.undo().map(drop),
                CanvasHistoryAction::Redo => document.redo().map(drop),
            }
        })
    }

    /// Enter applies the open modification's latest step.
    fn commit_modification(&mut self, now: Duration) -> Result<(), Box<dyn Error>> {
        let Some((target, modification)) = self.editing.modification else {
            return Ok(());
        };
        self.with_document(|tools, document| {
            tools.modify(target, modification, ModifyPhase::Apply, document, now)
        })
    }

    fn modify_coverage(
        &mut self,
        modify: CanvasCoverageModify,
        now: Duration,
    ) -> Result<(), Box<dyn Error>> {
        let target = match modify.target {
            CanvasModifyTarget::Selection => ModifyTarget::Selection,
            CanvasModifyTarget::Mask => ModifyTarget::Mask,
        };
        let operation = match modify.operation {
            CanvasModifyOperation::Expand => CoverageOperation::Expand,
            CanvasModifyOperation::Contract => CoverageOperation::Contract,
            CanvasModifyOperation::Feather => CoverageOperation::Feather,
        };
        let phase = match modify.phase {
            CanvasModifyPhase::Preview => ModifyPhase::Preview,
            CanvasModifyPhase::Apply => ModifyPhase::Apply,
            CanvasModifyPhase::Cancel => ModifyPhase::Cancel,
        };
        let modification =
            CoverageModification::new(operation, u32::from(modify.pixels)).map_err(refused)?;
        self.with_document(|tools, document| {
            tools.modify(target, modification, phase, document, now)
        })?;
        self.editing.modification =
            (phase == ModifyPhase::Preview).then_some((target, modification));
        Ok(())
    }
}
