// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The Input and Output routes swap whole documents; input documents outlive both routes.

use super::*;
use crate::native_canvas::session::inputs::InputRejection;

/// Retained-document storage for the small documents these tests render.
const FUNDED: u64 = 8 * 1024 * 1024;

/// The asset the current document's source shows.
fn shown_asset(session: &Session) -> Option<sugarsubstitute_image_ops::AssetId> {
    session.document.scene.as_ref()?.source.as_ref().map(|source| source.descriptor().asset())
}

fn parked_asset(session: &Session) -> Option<sugarsubstitute_image_ops::AssetId> {
    session
        .inputs
        .parked_output
        .as_ref()?
        .scene
        .as_ref()?
        .source
        .as_ref()
        .map(|source| source.descriptor().asset())
}

impl Fixture {
    /// Offer an encoded output and wait until the session has taken it.
    fn offer_output(&mut self, color: [u8; 3]) -> Result<(), Box<dyn Error>> {
        let mut output = Some(self.encoded(color)?);
        if !self.session.offer_source(&mut output, CacheReuse::Compatible, None)? {
            return Err("The output was not accepted".into());
        }
        self.settle_inputs()
    }
}

fn red_surface(directory: &Path) -> Result<SurfaceSpec, Box<dyn Error>> {
    let red = image(directory, "red.png", Extent::new(4, 3)?, |_, _| [255, 0, 0, 255])?;
    Ok(surface(
        "image",
        SurfaceBase::File(file("input/red.png", &red).0),
        vec![mask("first", None)?, mask("second", None)?],
    ))
}

#[test]
fn routes_swap_documents_both_ways_and_activation_waits_for_its_document()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    {
        // Documents and snapshots hold the canvas's retained memory until they drop.
        let directory = fixture._directory.path().to_path_buf();
        fixture.offer_output([0, 0, 255])?;
        let output = shown_asset(&fixture.session).ok_or("The output is not shown")?;
        let ticket = fixture.session.sync_inputs(WORKFLOW, vec![red_surface(&directory)?])?;
        fixture.session.activate_input(WORKFLOW, "image", Some("second"))?;
        assert!(!fixture.session.input_shown(), "the document is still loading");
        fixture.settle_inputs()?;
        let replies = fixture.session.take_input_replies();
        let reply = replies.iter().find(|reply| reply.ticket == ticket).ok_or("No sync reply")?;
        let image_surface = synced(reply, "image")?.clone();
        let session = &fixture.session;
        assert_eq!(session.document.input, Some(image_surface.document));
        assert_eq!(session.document.tools.target(), Some(layer(&image_surface, "second")?));
        assert_eq!(
            session.document.scene.as_ref().map(|scene| scene.source_revision),
            Some(session.selection),
            "the shown input document draws as the current selection"
        );
        assert_eq!(parked_asset(session), Some(output), "the output waits in its own slot");
        assert_eq!(session.retained.count(), 0, "it is not evictable retention");
        // Choosing another mask of the shown document changes the target at once.
        fixture.session.activate_input(WORKFLOW, "image", Some("first"))?;
        assert_eq!(fixture.session.document.tools.target(), Some(layer(&image_surface, "first")?));
        let unknown = fixture.session.activate_input(WORKFLOW, "image", Some("third"));
        assert!(unknown.is_err_and(|cause| matches!(
            cause.downcast_ref::<InputRejection>(),
            Some(InputRejection::Unknown)
        )));

        assert!(fixture.session.show_output(), "the parked output returns without a new offer");
        fixture.settle_inputs()?;
        assert!(!fixture.session.input_shown());
        assert_eq!(shown_asset(&fixture.session), Some(output));
        assert!(fixture.session.inputs.parked_output.is_none());
        assert!(matches!(
            fixture.session.input_snapshot(image_surface.document, None),
            InputSnapshot::Current(_)
        ));
        assert!(fixture.session.show_output(), "Output is already shown");

        // An output offered while the Input route shows waits behind the input document.
        fixture.session.activate_input(WORKFLOW, "image", None)?;
        fixture.settle_inputs()?;
        assert_eq!(fixture.session.document.tools.target(), None);
        fixture.offer_output([0, 255, 0])?;
        let session = &fixture.session;
        assert_eq!(session.document.input, Some(image_surface.document));
        assert_eq!(
            session.document.scene.as_ref().map(|scene| scene.source_revision),
            Some(session.selection)
        );
        let newer = parked_asset(session).ok_or("The newer output is not parked")?;
        assert_ne!(newer, output);
        assert!(fixture.session.show_output());
        fixture.settle_inputs()?;
        assert_eq!(shown_asset(&fixture.session), Some(newer));
    }
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn captures_read_parked_documents_and_report_changing_and_retired_ones()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    {
        // Documents and snapshots hold the canvas's retained memory until they drop.
        let directory = fixture._directory.path().to_path_buf();
        let canvas =
            surface("canvas", SurfaceBase::Blank(Extent::new(5, 5)?), vec![mask("mask", None)?]);
        let reply = fixture.sync(vec![red_surface(&directory)?, canvas.clone()])?;
        let image_surface = synced(&reply, "image")?.clone();
        let blank = synced(&reply, "canvas")?.clone();
        fixture.session.activate_input(WORKFLOW, "image", Some("first"))?;
        fixture.settle_inputs()?;
        let revision = fixture.snapshot(blank.document)?.revision();
        let parked = fixture.session.input_snapshot(blank.document, Some(revision));
        assert!(
            matches!(&parked, InputSnapshot::Current(snapshot) if snapshot.identity() == blank.document.get())
        );
        let current = fixture.session.input_snapshot(image_surface.document, None);
        assert!(matches!(current, InputSnapshot::Current(_)));
        assert!(matches!(
            fixture.session.input_snapshot(blank.document, Some(revision - 1)),
            InputSnapshot::Stale
        ));
        // Pending workflow work makes the document stale until it settles.
        let green = image(&directory, "green.png", Extent::new(4, 3)?, |_, _| [0, 255, 0, 255])?;
        let changed = surface(
            "image",
            SurfaceBase::File(file("input/green.png", &green).0),
            vec![mask("first", None)?, mask("second", None)?],
        );
        fixture.session.sync_inputs(WORKFLOW, vec![changed.clone(), canvas])?;
        assert!(matches!(
            fixture.session.input_snapshot(image_surface.document, None),
            InputSnapshot::Stale
        ));
        fixture.settle_inputs()?;
        assert!(matches!(
            fixture.session.input_snapshot(image_surface.document, None),
            InputSnapshot::Current(_)
        ));
        fixture.sync(vec![changed])?;
        assert!(matches!(
            fixture.session.input_snapshot(blank.document, None),
            InputSnapshot::Unavailable
        ));
    }
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn retiring_the_shown_surface_returns_to_output_and_close_retires_every_document()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    let directory = fixture._directory.path().to_path_buf();
    let wide = image(&directory, "wide.png", Extent::new(6, 5)?, |_, _| [1, 1, 1, 255])?;
    let (queued, queued_source) = file("input/wide.png", &wide);
    {
        // Documents and snapshots hold the canvas's retained memory until they drop.
        let reply = fixture.sync(vec![
            red_surface(&directory)?,
            surface("canvas", SurfaceBase::Blank(Extent::new(5, 5)?), vec![mask("mask", None)?]),
        ])?;
        let blank = synced(&reply, "canvas")?.clone();
        fixture.session.activate_input(WORKFLOW, "canvas", Some("mask"))?;
        fixture.settle_inputs()?;
        assert_eq!(fixture.session.document.input, Some(blank.document));
        fixture.sync(vec![red_surface(&directory)?])?;
        assert!(!fixture.session.input_shown(), "a retired surface leaves the screen");
        assert!(matches!(
            fixture.session.input_snapshot(blank.document, None),
            InputSnapshot::Unavailable
        ));
        // Close with a shown input document, a parked output and work still queued.
        fixture.session.activate_input(WORKFLOW, "image", None)?;
        fixture.settle_inputs()?;
        fixture.offer_output([3, 4, 5])?;
        fixture
            .session
            .sync_inputs(WORKFLOW, vec![surface("image", SurfaceBase::File(queued), Vec::new())])?;
    }
    let _directory = fixture.finish()?;
    assert!(queued_source.settled(), "every adapter retires before close completes");
    Ok(())
}

#[test]
fn retained_pressure_releases_input_backings_but_never_input_documents()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::funded(false, FUNDED)?;
    {
        // Documents and snapshots hold the canvas's retained memory until they drop.
        fixture.session.resize(Extent::new(8, 8)?)?;
        let directory = fixture._directory.path().to_path_buf();
        let reply = fixture.sync(vec![red_surface(&directory)?])?;
        let image_surface = synced(&reply, "image")?.clone();
        fixture.session.activate_input(WORKFLOW, "image", Some("first"))?;
        fixture.settle_inputs()?;
        assert!(fixture.session.document.fallback_front.is_some(), "the input document rendered");
        assert!(fixture.session.show_output(), "the empty document shown before returns");
        fixture.settle_inputs()?;
        let parked = fixture.session.inputs.bytes();
        assert!(parked > 0, "the parked input document keeps its backings");
        assert_eq!(fixture.session.retained_bytes(), parked, "the budget sees them");
        fixture.session.set_retained_limit(0);
        assert_eq!(fixture.session.retained_bytes(), 0);
        assert!(matches!(
            fixture.session.input_snapshot(image_surface.document, None),
            InputSnapshot::Current(_)
        ));
        fixture.settle_inputs()?;
        fixture.session.set_retained_limit(u64::MAX);
        fixture.session.activate_input(WORKFLOW, "image", Some("first"))?;
        fixture.settle_inputs()?;
        assert_eq!(fixture.session.document.input, Some(image_surface.document));
        assert!(fixture.session.document.fallback_front.is_some(), "it renders again when shown");
    }
    let _directory = fixture.finish()?;
    Ok(())
}
