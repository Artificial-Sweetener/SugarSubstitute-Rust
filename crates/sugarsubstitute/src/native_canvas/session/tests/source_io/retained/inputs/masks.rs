// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Mask files load through ComfyUI's `LoadImageMask` channel rules and never shrink or clear.

use super::*;
use crate::native_canvas::session::inputs::InitialOutcome;
use sugarsubstitute_image_ops::coverage_from_u8;

/// A 3x2 RGBA pattern whose channels all differ, transparent in places.
fn sample(x: u32, y: u32) -> [u8; 4] {
    let index = (y * 3 + x) as u8;
    [index * 40 + 5, 250 - index * 30, index * 17, [255, 0, 128, 64, 1, 200][index as usize]]
}

fn outcome<'a>(surface: &'a SyncedSurface, key: &str) -> Option<&'a InitialOutcome> {
    surface.masks.iter().find(|mask| &*mask.key == key).and_then(|mask| mask.initial.as_ref())
}

#[test]
fn initial_files_load_through_their_channel_and_other_sizes_are_rejected()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    {
        // Documents and snapshots hold the canvas's retained memory until they drop.
        let directory = fixture._directory.path().to_path_buf();
        let extent = Extent::new(3, 2)?;
        let base = image(&directory, "base.png", extent, |_, _| [9, 9, 9, 255])?;
        let pattern = image(&directory, "pattern.png", extent, sample)?;
        let small = image(&directory, "small.png", Extent::new(2, 2)?, |_, _| [255; 4])?;
        let missing = directory.join("missing.png");
        let channels = [
            ("red", MaskChannel::Red),
            ("green", MaskChannel::Green),
            ("blue", MaskChannel::Blue),
            ("alpha", MaskChannel::Alpha),
        ];
        let spec = |fixture_files: bool| -> Result<SurfaceSpec, Box<dyn Error>> {
            let mut masks = Vec::new();
            for (key, channel) in channels {
                let initial = fixture_files.then(|| file("input/pattern.png", &pattern).0);
                masks.push(MaskSpec { channel, ..mask(key, initial)? });
            }
            masks.push(mask("small", fixture_files.then(|| file("input/small.png", &small).0))?);
            masks.push(mask(
                "missing",
                fixture_files.then(|| file("input/missing.png", &missing).0),
            )?);
            Ok(surface("image", SurfaceBase::File(file("input/base.png", &base).0), masks))
        };
        let reply = fixture.sync(vec![spec(true)?])?;
        let loaded = synced(&reply, "image")?.clone();
        let snapshot = fixture.snapshot(loaded.document)?;
        for (key, channel) in channels {
            assert!(matches!(outcome(&loaded, key), Some(InitialOutcome::Loaded)), "{key}");
            let values = coverage(&snapshot, layer(&loaded, key)?)?;
            for y in 0..2 {
                for x in 0..3 {
                    let expected = coverage_from_u8(channel.read(sample(x, y)));
                    assert_eq!(values.value(x, y), expected, "{key} at {x},{y}");
                }
            }
        }
        let [red, _, _, alpha] = sample(0, 1);
        assert_eq!(MaskChannel::Red.read(sample(0, 1)), red);
        assert_eq!(MaskChannel::Alpha.read(sample(0, 1)), 255 - alpha, "alpha is inverted");
        assert!(matches!(
            outcome(&loaded, "small"),
            Some(InitialOutcome::Mismatch { expected, actual })
                if *expected == extent && *actual == Extent::new(2, 2)?
        ));
        assert_eq!(coverage(&snapshot, layer(&loaded, "small")?)?.uniform(), Some(0));
        assert!(matches!(
            outcome(&loaded, "missing"),
            Some(InitialOutcome::Failed(failure)) if !failure.to_string().is_empty()
        ));
        // Without a file the masks keep their content; the same file is not loaded again.
        let revision = snapshot.revision();
        let reply = fixture.sync(vec![spec(false)?])?;
        assert_eq!(synced(&reply, "image")?.revision, revision);
        let unchanged = coverage(&fixture.snapshot(loaded.document)?, layer(&loaded, "red")?)?;
        assert_eq!(unchanged.value(1, 0), coverage_from_u8(sample(1, 0)[0]));
        let (again, source) = file("input/pattern.png", &pattern);
        let mut masks = vec![mask("red", Some(again))?];
        masks.extend(
            ["green", "blue", "alpha", "small", "missing"]
                .map(|key| mask(key, None))
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?,
        );
        let reply = fixture.sync(vec![surface(
            "image",
            SurfaceBase::File(file("input/base.png", &base).0),
            masks,
        )])?;
        assert_eq!(source.acquisitions.load(Ordering::SeqCst), 0);
        assert_eq!(synced(&reply, "image")?.revision, revision);
        // A different file replaces the mask, and the load is not an undoable step.
        let full = image(&directory, "full.png", extent, |_, _| [255, 255, 255, 255])?;
        let mut masks = vec![mask("red", Some(file("input/full.png", &full).0))?];
        masks.extend(
            ["green", "blue", "alpha", "small", "missing"]
                .map(|key| mask(key, None))
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?,
        );
        let reply = fixture.sync(vec![surface(
            "image",
            SurfaceBase::File(file("input/base.png", &base).0),
            masks,
        )])?;
        let reloaded = synced(&reply, "image")?;
        assert!(matches!(outcome(reloaded, "red"), Some(InitialOutcome::Loaded)));
        let snapshot = fixture.snapshot(loaded.document)?;
        assert_eq!(coverage(&snapshot, layer(&loaded, "red")?)?.uniform(), Some(u16::MAX));
        let (state, _) =
            fixture.session.input_document_mut(loaded.document).ok_or("Document absent")?;
        assert!(!state.scene.as_mut().ok_or("Scene absent")?.document.undo()?);
    }
    let _directory = fixture.finish()?;
    Ok(())
}
