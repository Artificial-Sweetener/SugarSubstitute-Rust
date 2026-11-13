// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Appearance, hover emphasis and canvas resolution changes keep their history rules.

use super::*;
use crate::native_canvas::session::inputs::{
    AppearanceChange, InputRejection, ResizeAnchor, ResizeMode,
};
use sugarsubstitute_image_ops::{CoverageResampling, Region};

fn mask_appearance(
    snapshot: &DocumentSnapshot,
    layer: LayerId,
) -> Result<MaskAppearance, Box<dyn Error>> {
    snapshot
        .layers()
        .iter()
        .find(|candidate| candidate.id() == layer)
        .and_then(|candidate| match candidate.content() {
            LayerContent::Coverage { appearance, .. } => Some(*appearance),
            _ => None,
        })
        .ok_or_else(|| "The mask layer is absent".into())
}

fn undo(fixture: &mut Fixture, document: InputDocumentId) -> Result<bool, Box<dyn Error>> {
    let (state, _) = fixture.session.input_document_mut(document).ok_or("Document absent")?;
    Ok(state.scene.as_mut().ok_or("Scene absent")?.document.undo()?)
}

fn redo(fixture: &mut Fixture, document: InputDocumentId) -> Result<bool, Box<dyn Error>> {
    let (state, _) = fixture.session.input_document_mut(document).ok_or("Document absent")?;
    Ok(state.scene.as_mut().ok_or("Scene absent")?.document.redo()?)
}

#[test]
fn synced_appearance_applies_only_when_it_changes_and_commands_are_one_undo_step()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    {
        // Documents and snapshots hold the canvas's retained memory until they drop.
        let synced_colour = appearance([0, 120, 215], 0.5)?;
        let spec = |colour: MaskAppearance| -> Result<Vec<SurfaceSpec>, Box<dyn Error>> {
            Ok(vec![surface(
                "canvas",
                SurfaceBase::Blank(Extent::new(4, 4)?),
                vec![
                    MaskSpec { appearance: colour, ..mask("first", None)? },
                    MaskSpec { appearance: colour, ..mask("second", None)? },
                ],
            )])
        };
        let reply = fixture.sync(spec(synced_colour)?)?;
        let canvas = synced(&reply, "canvas")?.clone();
        let (first, second) = (layer(&canvas, "first")?, layer(&canvas, "second")?);
        let chosen = appearance([200, 30, 30], 0.8)?;
        fixture.session.set_input_appearance(
            WORKFLOW,
            &[
                AppearanceChange {
                    surface: Arc::from("canvas"),
                    mask: Arc::from("first"),
                    appearance: chosen,
                },
                AppearanceChange {
                    surface: Arc::from("canvas"),
                    mask: Arc::from("second"),
                    appearance: chosen,
                },
            ],
        )?;
        let snapshot = fixture.snapshot(canvas.document)?;
        assert_eq!(mask_appearance(&snapshot, first)?, chosen);
        assert_eq!(mask_appearance(&snapshot, second)?, chosen);
        // A sync repeating the last synced appearance leaves the command's appearance alone.
        fixture.sync(spec(synced_colour)?)?;
        assert_eq!(mask_appearance(&fixture.snapshot(canvas.document)?, first)?, chosen);
        // One undo reverts both masks: the command was a single step.
        assert!(undo(&mut fixture, canvas.document)?);
        let undone = fixture.snapshot(canvas.document)?;
        assert_eq!(mask_appearance(&undone, first)?, synced_colour);
        assert_eq!(mask_appearance(&undone, second)?, synced_colour);
        assert!(!undo(&mut fixture, canvas.document)?);
        // A changed synced appearance applies everywhere without becoming a step of its own.
        let resynced = appearance([10, 200, 10], 0.25)?;
        fixture.sync(spec(resynced)?)?;
        assert_eq!(mask_appearance(&fixture.snapshot(canvas.document)?, first)?, resynced);
        // Provisioning rewrote the undone command's state as well.
        assert!(redo(&mut fixture, canvas.document)?);
        assert_eq!(mask_appearance(&fixture.snapshot(canvas.document)?, first)?, resynced);
        let unknown = AppearanceChange {
            surface: Arc::from("canvas"),
            mask: Arc::from("none"),
            appearance: chosen,
        };
        let rejected = fixture.session.set_input_appearance(WORKFLOW, &[unknown]);
        assert!(rejected.is_err_and(|cause| matches!(
            cause.downcast_ref::<InputRejection>(),
            Some(InputRejection::Unknown)
        )));
    }
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn hover_emphasis_is_transient_and_never_part_of_history() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    {
        // Documents and snapshots hold the canvas's retained memory until they drop.
        let reply = fixture.sync(vec![surface(
            "canvas",
            SurfaceBase::Blank(Extent::new(4, 4)?),
            vec![mask("first", None)?, mask("second", None)?],
        )])?;
        let canvas = synced(&reply, "canvas")?.clone();
        let before = fixture.snapshot(canvas.document)?;
        fixture.session.hover_input_mask(WORKFLOW, "canvas", Some("second"))?;
        let hovered = fixture.snapshot(canvas.document)?;
        assert_eq!(hovered.highlight(), Some(layer(&canvas, "second")?));
        assert!(hovered.revision() > before.revision(), "emphasis redraws the document");
        assert_eq!(hovered.layers(), before.layers(), "content is untouched");
        assert!(!undo(&mut fixture, canvas.document)?, "emphasis is no history step");
        fixture.session.hover_input_mask(WORKFLOW, "canvas", None)?;
        assert_eq!(fixture.snapshot(canvas.document)?.highlight(), None);
        assert!(fixture.session.hover_input_mask(WORKFLOW, "canvas", Some("none")).is_err());
    }
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn blank_surfaces_resize_around_an_anchor_or_scale_every_mask_as_undoable_steps()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    {
        // Documents and snapshots hold the canvas's retained memory until they drop.
        let directory = fixture._directory.path().to_path_buf();
        let reply = fixture.sync(vec![
            surface("canvas", SurfaceBase::Blank(Extent::new(4, 4)?), vec![mask("mask", None)?]),
            surface(
                "image",
                SurfaceBase::File(
                    file(
                        "input/image.png",
                        &image(&directory, "image.png", Extent::new(2, 2)?, |_, _| [1, 2, 3, 255])?,
                    )
                    .0,
                ),
                Vec::new(),
            ),
        ])?;
        let canvas = synced(&reply, "canvas")?.clone();
        let mask_layer = layer(&canvas, "mask")?;
        let memory =
            fixture.session.resources.as_ref().ok_or("Canvas storage absent")?.memory.clone();
        {
            let (state, _) =
                fixture.session.input_document_mut(canvas.document).ok_or("Document absent")?;
            let scene = state.scene.as_mut().ok_or("Scene absent")?;
            let snapshot = scene.document.snapshot();
            let current = snapshot
                .layers()
                .iter()
                .find(|layer| layer.id() == mask_layer)
                .ok_or("Mask absent")?;
            let LayerContent::Coverage { coverage, appearance } = current.content() else {
                return Err("Mask is not coverage".into());
            };
            let mut painted = coverage.clone();
            painted.update(Region::new(0, 0, 1, 1)?, &memory, |_, _, _| u16::MAX)?;
            let content = LayerContent::Coverage { coverage: painted, appearance: *appearance };
            scene.document.apply(sugarsubstitute_canvas::DocumentEdit::Replace(
                current.clone().with_content(content),
            ))?;
        }
        let ticket = fixture.session.resize_input(
            WORKFLOW,
            "canvas",
            Extent::new(6, 8)?,
            ResizeMode::Canvas(ResizeAnchor::BottomRight),
        )?;
        fixture.settle_inputs()?;
        let replies = fixture.session.take_input_replies();
        let reply =
            replies.iter().find(|reply| reply.ticket == ticket).ok_or("Resize did not reply")?;
        let resized = synced(reply, "canvas")?;
        assert_eq!(resized.extent, Extent::new(6, 8)?);
        assert_ne!(resized.geometry, canvas.geometry);
        let snapshot = fixture.snapshot(canvas.document)?;
        let placed = coverage(&snapshot, mask_layer)?;
        assert_eq!(
            (placed.value(2, 4), placed.value(0, 0)),
            (u16::MAX, 0),
            "anchored bottom-right"
        );
        let LayerContent::VectorRectangle(backing) = snapshot.layers()[0].content() else {
            return Err("The blank backing is not a rectangle".into());
        };
        assert_eq!(backing.bounds(), [0.0, 0.0, 6.0, 8.0]);
        assert!(undo(&mut fixture, canvas.document)?);
        assert_eq!(fixture.snapshot(canvas.document)?.extent(), Extent::new(4, 4)?);
        assert!(redo(&mut fixture, canvas.document)?);
        let ticket = fixture.session.resize_input(
            WORKFLOW,
            "canvas",
            Extent::new(12, 16)?,
            ResizeMode::Scale(CoverageResampling::Nearest),
        )?;
        fixture.settle_inputs()?;
        assert!(fixture.session.take_input_replies().iter().any(|reply| reply.ticket == ticket));
        let scaled = coverage(&fixture.snapshot(canvas.document)?, mask_layer)?;
        assert_eq!(scaled.extent(), Extent::new(12, 16)?);
        assert_eq!(
            [scaled.value(4, 8), scaled.value(5, 9), scaled.value(6, 10)],
            [u16::MAX, u16::MAX, 0]
        );
        // Only blank-based surfaces change resolution; a later synced blank size is ignored.
        let rejected = fixture.session.resize_input(
            WORKFLOW,
            "image",
            Extent::new(4, 4)?,
            ResizeMode::Scale(CoverageResampling::Smooth),
        );
        assert!(rejected.is_err_and(|cause| matches!(
            cause.downcast_ref::<InputRejection>(),
            Some(InputRejection::Invalid(_))
        )));
        fixture.sync(vec![surface(
            "canvas",
            SurfaceBase::Blank(Extent::new(3, 3)?),
            vec![mask("mask", None)?],
        )])?;
        assert_eq!(fixture.snapshot(canvas.document)?.extent(), Extent::new(12, 16)?);
    }
    let _directory = fixture.finish()?;
    Ok(())
}
