// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native raster requests retain only source revision metadata, preserving all four real snapshot credits.
use super::*;
use sugarsubstitute_contracts::transcript::*;
#[test]
fn native_requests_release_unused_source_pins_before_real_cpu_admission_and_keep_revision_fences()
-> Result<(), Box<dyn Error>> {
    let (mut driver, wake, mut owner) = support::started()?;
    // Select native presentation while the genuine driver is still inert, before its first request.
    driver.console_presentation =
        sugarsubstitute_contracts::presentation::ConsolePresentation::Native;
    let mut store = support::transcript(1)?;
    for sequence in 1..=2 {
        store.apply(&OutputBatch::new(
            store.resource(),
            OutputStream::Stdout,
            StreamSequence::new(sequence)?,
            "actual 日本語😀\n".as_bytes(),
            None,
            false,
        )?)?;
        driver.request(
            FrameShape::new(558, 558, 1.0)?,
            Appearance::demo(),
            PaperFlip::new(1).frame(Duration::ZERO),
            ProgressFrame { completed: 0.5, activity: None },
            Some(store.snapshot(None, 128)?.into()),
        )?;
        assert_eq!(driver.console_binding, Some((store.resource(), sequence)));
        if let Some(pending) = driver.pending.as_mut() {
            let request = pending.input.take()?;
            assert!(request.console.is_none());
            assert_eq!(
                request.console_presentation,
                sugarsubstitute_contracts::presentation::ConsolePresentation::Native
            );
            pending.input = PreparationTransfer::new(request);
        }
        let pins = [
            store.snapshot(None, 128)?,
            store.snapshot(None, 128)?,
            store.snapshot(None, 128)?,
            store.snapshot(None, 128)?,
        ];
        assert!(store.snapshot(None, 128).is_err());
        drop(pins);
    }
    let frame = support::pending_frame(&mut driver, &wake)?;
    let native = frame
        .claim_storage(sugarsubstitute_contracts::presentation::NATIVE_CONSOLE_STORAGE_BYTES)?;
    support::finish(&mut owner, &mut driver, &wake)?;
    drop(frame);
    assert_eq!(driver.scope.snapshot().reserved_bytes, native.reserved_bytes());
    drop(native);
    assert_eq!(driver.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

/// Provisioning output cannot starve status pixels owned independently of native console text.
#[test]
fn native_output_updates_preserve_completed_status_frames() -> Result<(), Box<dyn Error>> {
    let (mut driver, wake, mut owner) = support::started()?;
    driver.console_presentation =
        sugarsubstitute_contracts::presentation::ConsolePresentation::Native;
    let bootstrap = support::prepare(&mut driver, &wake, 0.0)?;
    let mut store = support::transcript(1)?;
    support::request(&mut driver, 0.5)?;
    support::terminal(&driver, &wake)?;
    let original = driver
        .active
        .as_ref()
        .and_then(|active| active.identity)
        .ok_or("Status frame identity absent")?;
    for sequence in 1..=4 {
        store.apply(&OutputBatch::new(
            store.resource(),
            OutputStream::Stdout,
            StreamSequence::new(sequence)?,
            b"Provisioning dependency\n",
            None,
            false,
        )?)?;
        driver.request(
            FrameShape::new(558, 558, 1.0)?,
            Appearance::demo(),
            PaperFlip::new(1).frame(Duration::ZERO),
            ProgressFrame { completed: 0.5, activity: None },
            Some(store.snapshot_tail(1)?.into()),
        )?;
    }
    let frame = support::poll(&mut driver)?.ok_or("Output discarded completed status pixels")?;
    assert_eq!(frame.identity(), original);
    assert_eq!(driver.console_binding, Some((store.resource(), 4)));
    drop((bootstrap, frame));
    support::finish(&mut owner, &mut driver, &wake)?;
    Ok(())
}
