// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Close native preview ownership through actual worker disposal and complete resident release.
use super::*;

#[test]
fn closing_an_accepted_preview_releases_correlations_and_settles_the_original_execution()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    watch(&mut fixture, 100, true)?;
    transfer(&mut fixture, 200, "preview:1", JPEG)?;
    let target = wait_frame(&mut fixture)?;
    fixture.canvas.presented(target)?;
    wait_change(&mut fixture, "presented")?;
    let memory = fixture
        .canvas
        .session
        .as_ref()
        .and_then(Session::preview_resident)
        .ok_or("Accepted preview resident owner is absent")?;
    assert!(memory.claim(memory.bytes()).is_err());
    drain(&mut fixture)?;
    fixture.finish_resident(memory)
}

#[test]
fn closing_an_incomplete_transfer_retires_its_storage_before_releasing_resident_claims()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    watch(&mut fixture, 100, true)?;
    begin(&mut fixture, 200, "preview:1", 1024 * 1024)?;
    let memory = fixture
        .canvas
        .session
        .as_ref()
        .and_then(Session::preview_resident)
        .ok_or("Staged preview resident owner is absent")?;
    assert!(!fixture.canvas.surface.preview_custody_settled());
    assert!(memory.claim(memory.bytes()).is_err());
    drain(&mut fixture)?;
    fixture.finish_resident(memory)
}
