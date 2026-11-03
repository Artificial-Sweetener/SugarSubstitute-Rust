// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Paint and erase the target mask layer in strokes, as CuteCanvas's Brush and Eraser do.
//!
//! A stroke starts on the primary button and is one edit session: every pointer sample adds the
//! dabs between it and the previous contact, then stages the updated mask so the preview renders
//! it live, and release commits the stroke as one undo step. Cancelling discards it; Space
//! commits it before temporary navigation takes over. Pen pressure scales the diameter down to
//! 15% of the size; Alt or a pen's eraser end turns the Brush into an eraser; with a selection
//! present, strokes are clipped to it. Shift with a mouse press paints a straight line from the
//! end of the previous stroke as a stroke of its own, and while Shift is held a mouse drag does
//! not paint. The mouse wheel changes the size by five pixels a notch, and the middle button
//! pans.
mod options;
pub use options::BrushOptions;

use super::*;
use crate::{
    BrushOutline, Layer, LayerContent, MaskAppearance,
    paint::{self, Contact, DabStyle, StrokeRaster},
};

/// Whether a brush adds or removes mask coverage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrushMode {
    /// Paint coverage; Alt or a pen's eraser end erases instead.
    Paint,
    /// Always erase.
    Erase,
}

/// The Brush or Eraser tool. It edits the layer [`ToolManager::set_target`] names when that
/// layer is an unlocked mask, and does nothing otherwise.
pub struct BrushTool {
    mode: BrushMode,
    options: BrushOptions,
    pointer: Option<PointerSample>,
    stroke: Option<Stroke>,
    /// Where the last stroke ended, in document pixels: the start of a Shift-click line.
    anchor: Option<[f64; 2]>,
    wheel: f64,
}

/// The stroke in progress: the mask layer as it was when the stroke began, and its working
/// coverage.
struct Stroke {
    layer: Layer,
    appearance: MaskAppearance,
    raster: StrokeRaster,
    previous: Contact,
}

impl BrushTool {
    /// A brush in `mode` with `options`.
    pub const fn new(mode: BrushMode, options: BrushOptions) -> Self {
        Self { mode, options, pointer: None, stroke: None, anchor: None, wheel: 0.0 }
    }
    /// Whether this tool paints or erases.
    pub const fn mode(&self) -> BrushMode {
        self.mode
    }

    fn begin(
        &mut self,
        modifiers: ToolModifiers,
        context: &mut ToolContext<'_>,
    ) -> Result<(), CanvasError> {
        let (Some(sample), None) = (self.pointer, self.stroke.as_ref()) else {
            return Ok(());
        };
        let Some(document) = context.document() else {
            return Ok(());
        };
        let Some((layer, coverage, appearance)) = mask(&document, context.target()) else {
            return Ok(());
        };
        let layer = layer.clone();
        let selection = document.selection().cloned();
        let raster = StrokeRaster::new(coverage.clone(), selection, context.memory()?)?;
        let contact = self.contact(sample, context)?;
        let line = self.anchor.filter(|_| sample.kind == PointerKind::Mouse && modifiers.shift);
        let start = line.map_or(contact, |anchor| Contact::new(anchor, contact.diameter));
        context.begin_edit()?;
        let mut stroke = Stroke { layer, appearance, raster, previous: contact };
        let style = self.style(modifiers.alt || sample.eraser);
        if let Err(cause) = stroke.paint(paint::segment(start, contact, false), style, context) {
            context.cancel_edit();
            return Err(cause);
        }
        self.anchor = Some(contact.position);
        if line.is_some() {
            context.commit_edit()?;
        } else {
            context.capture_pointer(true);
            self.stroke = Some(stroke);
        }
        Ok(())
    }

    fn extend(
        &mut self,
        sample: PointerSample,
        context: &mut ToolContext<'_>,
    ) -> Result<(), CanvasError> {
        // CuteCanvas previews a Shift line during a mouse drag instead of painting.
        if sample.kind == PointerKind::Mouse && sample.modifiers.shift && self.anchor.is_some() {
            return Ok(());
        }
        let contact = self.contact(sample, context)?;
        let style = self.style(sample.modifiers.alt || sample.eraser);
        let Some(stroke) = self.stroke.as_mut() else {
            return Ok(());
        };
        if contact == stroke.previous {
            return Ok(());
        }
        let dabs = paint::segment(stroke.previous, contact, true);
        stroke.previous = contact;
        self.anchor = Some(contact.position);
        if let Err(cause) = stroke.paint(dabs, style, context) {
            self.stroke = None;
            context.cancel_edit();
            return Err(cause);
        }
        Ok(())
    }

    fn finish(&mut self, context: &mut ToolContext<'_>) -> Result<(), CanvasError> {
        if self.stroke.take().is_some() {
            context.commit_edit()?;
        }
        Ok(())
    }

    /// The contact a pointer sample makes: its document position and, for a pen, the
    /// pressure-scaled diameter.
    fn contact(
        &self,
        sample: PointerSample,
        context: &ToolContext<'_>,
    ) -> Result<Contact, CanvasError> {
        let position = context.navigation().viewport().document_at(sample.position)?;
        let size = f64::from(self.options.size());
        let diameter = match sample.kind {
            PointerKind::Pen => paint::pressure_diameter(size, sample.pressure.get()),
            PointerKind::Mouse | PointerKind::Touch => size,
        };
        Ok(Contact::new(position, diameter))
    }

    fn style(&self, erase_requested: bool) -> DabStyle {
        DabStyle {
            hardness: self.options.hardness(),
            opacity: self.options.opacity(),
            erase: self.mode == BrushMode::Erase || erase_requested,
        }
    }

    /// Change the size by whole wheel notches, carrying fractional notches within a gesture.
    fn wheel(&mut self, steps: f64, phase: ScrollPhase) {
        if phase == ScrollPhase::Begin {
            self.wheel = 0.0;
        }
        if steps.is_finite() {
            self.wheel += steps;
            let notches = self.wheel.trunc();
            self.wheel -= notches;
            self.options = self.options.resized_by(notches as i64);
        }
        if phase == ScrollPhase::End {
            self.wheel = 0.0;
        }
    }
}

impl Stroke {
    /// Composite a segment's dabs and stage the updated mask in the edit session.
    fn paint(
        &mut self,
        dabs: paint::SegmentDabs,
        style: DabStyle,
        context: &mut ToolContext<'_>,
    ) -> Result<(), CanvasError> {
        for dab in dabs {
            self.raster.stamp(dab, style)?;
        }
        let coverage = self.raster.flush()?.clone();
        let content = LayerContent::Coverage { coverage, appearance: self.appearance };
        context.stage(DocumentEdit::Replace(self.layer.clone().with_content(content)))
    }
}

/// The unlocked mask layer `target` names, with its coverage and appearance.
fn mask(
    document: &DocumentSnapshot,
    target: Option<LayerId>,
) -> Option<(&Layer, &sugarsubstitute_image_ops::Coverage, MaskAppearance)> {
    let layer = document.layers().iter().find(|layer| Some(layer.id()) == target)?;
    match layer.content() {
        LayerContent::Coverage { coverage, appearance } if !layer.locked() => {
            Some((layer, coverage, *appearance))
        }
        _ => None,
    }
}

impl CanvasTool for BrushTool {
    fn input(
        &mut self,
        input: ToolInput,
        context: &mut ToolContext<'_>,
    ) -> Result<(), CanvasError> {
        match input {
            ToolInput::Pointer(sample) => {
                context.navigate(NavigationInput::Pointer(sample.position))?;
                self.pointer = Some(sample);
                if self.stroke.is_some() {
                    self.extend(sample, context)?;
                }
                Ok(())
            }
            ToolInput::Button { button: ToolButton::Primary, pressed: true, modifiers } => {
                self.begin(modifiers, context)
            }
            ToolInput::Button { button: ToolButton::Primary, pressed: false, .. } => {
                self.finish(context)
            }
            ToolInput::Button { button: ToolButton::Middle, pressed, .. } => {
                context.navigate(NavigationInput::Pan(pressed))
            }
            ToolInput::Button { button: ToolButton::Secondary, .. } | ToolInput::DoubleClick(_) => {
                Ok(())
            }
            ToolInput::Wheel { steps, phase } => {
                self.wheel(steps, phase);
                Ok(())
            }
            ToolInput::Fit => context.navigate(NavigationInput::Fit),
            ToolInput::OneToOne => context.navigate(NavigationInput::OneToOne),
        }
    }

    fn suspend(&mut self, context: &mut ToolContext<'_>) -> Result<(), CanvasError> {
        self.finish(context)
    }

    fn cancel(&mut self, reason: ToolCancelReason) {
        self.stroke = None;
        self.wheel = 0.0;
        // Leaving the tool or the document forgets where the last stroke ended.
        if matches!(
            reason,
            ToolCancelReason::SelectionChanged
                | ToolCancelReason::DocumentChanged
                | ToolCancelReason::Shutdown
        ) {
            self.anchor = None;
        }
    }

    /// The brush ring at the on-screen brush size in the target mask's colour, or the arrow when
    /// there is no mask to paint or the ring would not fit the viewport. A middle-button pan
    /// shows the closed hand.
    fn cursor(&self, view: &ToolView<'_>) -> CursorIntent {
        if view.navigation.captured() {
            return view.navigation.cursor();
        }
        let Some((_, _, appearance)) =
            view.document.as_ref().and_then(|document| mask(document, view.target))
        else {
            return CursorIntent::Arrow;
        };
        let viewport = view.navigation.viewport();
        let surface = viewport.surface();
        let diameter = f64::from(self.options.size()) * viewport.zoom();
        let limit = surface.width().min(surface.height()).min(BrushOutline::MAX_DIAMETER);
        if diameter.is_nan() || diameter > f64::from(limit) {
            return CursorIntent::Arrow;
        }
        let erase = self.mode == BrushMode::Paint
            && self.pointer.is_some_and(|pointer| pointer.modifiers.alt || pointer.eraser);
        CursorIntent::BrushOutline(BrushOutline::new(
            diameter.round() as u32,
            appearance.color(),
            erase,
        ))
    }

    fn options(&self) -> Option<ToolOptions> {
        Some(ToolOptions::Brush(self.options))
    }

    fn configure(&mut self, options: ToolOptions) -> Result<(), CanvasError> {
        let ToolOptions::Brush(options) = options;
        self.options = options;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
