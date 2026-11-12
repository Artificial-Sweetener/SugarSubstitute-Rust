// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Receipt promotion preserves authored current and parked output documents and their history.
use super::*;
use crate::native_canvas::session::RetentionKey;
use sugarsubstitute_canvas::DocumentEdit;

/// Edit the actual output document before its live identity acquires a durable receipt.
fn authored_output(fixture: &mut Fixture) -> Result<([u8; 32], u64, u64), Box<dyn Error>> {
    fixture.offer_output([0, 0, 255])?;
    let scene = fixture.session.document.scene.as_mut().ok_or("Output scene absent")?;
    let hash = scene.source.as_ref().ok_or("Output original absent")?.descriptor().original_hash();
    let snapshot = scene.document.snapshot();
    let layer = snapshot
        .layers()
        .first()
        .ok_or("Output layer absent")?
        .clone()
        .with_name("Authored output")?;
    scene.document.apply(DocumentEdit::Replace(layer))?;
    let snapshot = scene.document.snapshot();
    fixture.session.document.retention = Some(RetentionKey::new("live/backend"));
    Ok((hash, snapshot.identity(), snapshot.revision()))
}

#[test]
fn verified_original_rekeys_current_authored_output_without_replacing_history_or_camera()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    {
        let (hash, identity, revision) = authored_output(&mut fixture)?;
        let viewport = fixture.session.document.tools.navigation().viewport();
        let key = RetentionKey::new("verified/original");
        assert!(fixture.session.retain_output_original(&key, &[hash])?);
        let document = &mut fixture.session.document;
        assert_eq!(document.retention.as_ref(), Some(&key));
        assert_eq!(document.tools.navigation().viewport(), viewport);
        let scene = document.scene.as_mut().ok_or("Output scene lost")?;
        let snapshot = scene.document.snapshot();
        assert_eq!((snapshot.identity(), snapshot.revision()), (identity, revision));
        assert_eq!(snapshot.layers()[0].name(), "Authored output");
        assert!(scene.document.undo()?);
        assert_ne!(scene.document.snapshot().layers()[0].name(), "Authored output");
        assert!(scene.document.redo()?);
        assert_eq!(scene.document.snapshot().layers()[0].name(), "Authored output");
    }
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn parked_authored_output_refuses_wrong_original_and_returns_only_after_exact_promotion()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    {
        let (hash, identity, revision) = authored_output(&mut fixture)?;
        let directory = fixture._directory.path().to_path_buf();
        fixture.sync(vec![red_surface(&directory)?])?;
        fixture.session.activate_input(WORKFLOW, "image", None)?;
        fixture.settle_inputs()?;
        let old = RetentionKey::new("live/backend");
        let key = RetentionKey::new("verified/original");
        assert!(fixture.session.retain_output_original(&key, &[[0; 32]]).is_err());
        assert!(fixture.session.input_shown());
        let parked = fixture.session.inputs.parked_output.as_ref().ok_or("Authored output lost")?;
        assert_eq!(parked.retention.as_ref(), Some(&old));
        let snapshot = parked.scene.as_ref().ok_or("Authored scene lost")?.document.snapshot();
        assert_eq!((snapshot.identity(), snapshot.revision()), (identity, revision));
        assert_eq!(snapshot.layers()[0].name(), "Authored output");
        fixture.session.show_output_original(key.clone());
        assert!(!fixture.session.advance_input_route()?);
        assert!(fixture.session.input_shown(), "Unverified parked output was revealed");
        assert!(fixture.session.retain_output_original(&key, &[hash])?);
        fixture.session.show_output_original(key);
        fixture.settle_inputs()?;
        assert!(!fixture.session.input_shown());
        let scene = fixture.session.document.scene.as_mut().ok_or("Output scene lost")?;
        assert_eq!(scene.document.snapshot().identity(), identity);
        assert_eq!(scene.document.snapshot().layers()[0].name(), "Authored output");
        assert!(scene.document.undo()?);
        assert!(scene.document.redo()?);
    }
    let _directory = fixture.finish()?;
    Ok(())
}
