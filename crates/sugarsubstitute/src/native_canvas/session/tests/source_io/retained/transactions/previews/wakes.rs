// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify one-shot application demand and completion-driven retries without unrelated input.

use super::*;

#[test]
fn a_refused_preview_offer_requests_one_poll_then_retries_on_actual_capacity_release()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    fixture.session.resize(Extent::new(8, 8)?)?;
    fixture.session.input(Input::Drawable(true))?;
    let path = fixture._directory.path().join("wake-preview.png");
    let spec = fixture.session.spec(
        Operation::Infrastructure,
        JobClass::InfrastructureOrdinary,
        96 * 1024 * 1024,
        Duration::from_secs(30),
        None,
    )?;
    let scope = &fixture.session.runtime.as_ref().ok_or("Runtime absent")?.scope;
    let mut producer = fixture.client.submit_blocking(scope, spec, move |context| {
        let file = std::fs::File::create(&path)?;
        let mut encoder = png::Encoder::new(file, 1, 1);
        encoder.set_color(png::ColorType::Rgb);
        let mut writer = encoder.write_header().map_err(std::io::Error::other)?;
        writer.write_image_data(&[0, 0, 255]).map_err(std::io::Error::other)?;
        writer.finish().map_err(std::io::Error::other)?;
        Ok::<_, WorkError>((ImageSource::file(path), context.memory().clone()))
    })?;
    let completion = fixture.wake.completion(&fixture.client, &mut producer)?;
    let (source, retained) = Session::terminal(completion.outcome)?;
    drop(completion.memory);
    drop(producer);
    let now = fixture.session.navigation_time();
    let deadline = fixture.session.preview_now().checked_add(Duration::from_secs(30))?;
    let token = fixture
        .session
        .offer_preview(&mut Some(source), deadline)?
        .token()
        .ok_or("Preview refused")?;
    assert_eq!(fixture.session.preview_deadline(now)?, Some(now));
    fixture.session.poll(Duration::ZERO)?;
    assert_eq!(fixture.session.preview_status(token), PreviewStatus::Preparing);
    assert_eq!(
        fixture.session.preview_deadline(now)?,
        Some(Duration::from_nanos(deadline.as_nanos()))
    );
    fixture.session.retire(Retired { _fixture_memory: Some(retained), ..Default::default() })?;
    let observed = fixture.wake.revision();
    fixture.session.poll(Duration::ZERO)?;
    fixture.wake.wait_after(observed)?;
    let target = fixture.offered_preview(token)?;
    fixture.session.did_present_cpu(target)?;
    assert_eq!(fixture.session.preview_status(token), PreviewStatus::Presented(target));
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn an_idle_external_stage_abort_requests_cleanup_poll_without_renewing_the_close_cutoff()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    let memory = fixture.session.preview_resident().ok_or("Resident admission absent")?;
    let cutoff = fixture.session.preview_now().checked_add(Duration::from_secs(1))?;
    fixture.session.adopt_preview_retirement_cutoff(cutoff);
    let mut bytes = Some((Vec::with_capacity(64), memory.claim(64 + 128)?));
    let now = fixture.session.navigation_time();
    assert!(fixture.session.retire_preview_bytes(&mut bytes)?);
    assert_eq!(fixture.session.preview_deadline(now)?, Some(now));
    assert_eq!(fixture.session.retirement_deadline, Some(cutoff));
    fixture.session.poll(Duration::ZERO)?;
    assert_ne!(fixture.session.preview_deadline(now)?, Some(now));
    fixture.preview_idle()?;
    assert!(fixture.session.retirement.is_none());
    assert!(fixture.session.pending_retirement.is_none());
    assert_eq!(fixture.session.shutdown_deadline, Some(cutoff));
    drop(memory);
    let _directory = fixture.finish()?;
    Ok(())
}
