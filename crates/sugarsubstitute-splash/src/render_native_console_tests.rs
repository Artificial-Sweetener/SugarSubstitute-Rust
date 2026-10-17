// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify native text storage admission through the actual central raster execution path.
use super::*;
use sugarsubstitute_contracts::presentation::{ConsolePresentation, NATIVE_CONSOLE_STORAGE_BYTES};
#[test]
fn native_console_frame_admits_distinct_copy_and_keeps_full_pin_without_raster_text() -> TestResult
{
    let rig = Rig::new()?;
    let renderer = initialize(&rig)?;
    let store = support::transcript()?;
    let base = request(1.0, true)?;
    let expected = base.frame();
    let request = SplashRenderRequest {
        shape: base.shape,
        identity: base.identity,
        appearance: base.appearance,
        character: base.character,
        feedback: base.feedback,
        bootstrap: base.bootstrap,
        console: Some(store.snapshot(None, 128)?),
        console_presentation: ConsolePresentation::Native,
    };
    let bytes = request.reservation_bytes();
    assert_eq!(bytes, base.reservation_bytes() + NATIVE_CONSOLE_STORAGE_BYTES);
    let mut job = rig.client.submit_cpu(&rig.scope, spec(2, bytes)?, move |context| {
        renderer.prepare(request, context.memory().clone())
    })?;
    let result = success(rig.completion(&mut job)?)?;
    assert_eq!(result.console_clipped_rows, 0);
    assert_eq!(
        result
            .console
            .as_ref()
            .ok_or("actual native source lost")?
            .records()
            .next()
            .ok_or("original text lost")?
            .text(),
        "original 日本語😀"
    );
    let frame = result.frame?;
    let mut offline = SplashRenderer::new("en")?;
    let pixels = offline.render(
        expected.width,
        expected.height,
        expected.scale,
        expected.appearance,
        expected.character,
        expected.feedback,
    )?;
    assert_eq!(frame.pixels(), pixels.data());
    let native = frame.claim_storage(NATIVE_CONSOLE_STORAGE_BYTES)?;
    let dib = frame.claim_storage(frame.shape().byte_len() as u64)?;
    drop((frame, result.renderer, result.console, job));
    assert_eq!(rig.scope.snapshot().reserved_bytes, bytes);
    drop(dib);
    assert_eq!(rig.scope.snapshot().reserved_bytes, bytes);
    drop(native);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn native_console_allowance_is_separate_from_both_pixel_copies_at_every_supported_scale()
-> TestResult {
    for scale in [0.5, 1.0, 1.25, 2.0, 4.0] {
        let base = request(scale, true)?;
        let request = SplashRenderRequest {
            shape: base.shape,
            identity: base.identity,
            appearance: base.appearance,
            character: base.character,
            feedback: base.feedback,
            bootstrap: base.bootstrap,
            console: None::<sugarsubstitute_application::transcript::TranscriptSnapshot>,
            console_presentation: ConsolePresentation::Native,
        };
        assert!(
            request.reservation_bytes()
                >= 2 * base.shape.byte_len() as u64 + NATIVE_CONSOLE_STORAGE_BYTES
        );
        assert_eq!(
            request.reservation_bytes() - base.reservation_bytes(),
            NATIVE_CONSOLE_STORAGE_BYTES
        );
    }
    Ok(())
}
