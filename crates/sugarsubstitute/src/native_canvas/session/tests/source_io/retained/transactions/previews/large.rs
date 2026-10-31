// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the full retained fallback patch through actual image import and session admission.

use super::*;

#[test]
fn a_full_size_preview_admits_its_complete_patch_and_physical_projection_work()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    fixture.session.resize(Extent::new(8, 8)?)?;
    fixture.session.input(Input::Drawable(true))?;
    let path = fixture._directory.path().join("full-size-preview.png");
    let spec = fixture.session.spec(
        Operation::Infrastructure,
        JobClass::InfrastructureOrdinary,
        32 * 1024 * 1024,
        Duration::from_secs(30),
        None,
    )?;
    let scope = &fixture.session.runtime.as_ref().ok_or("Runtime absent")?.scope;
    let mut job = fixture.client.submit_blocking(scope, spec, move |context| {
        let _working = context.memory().claim(16 * 1024 * 1024)?;
        let mut pixels = Vec::new();
        pixels.try_reserve_exact(1024 * 1024 * 3).map_err(std::io::Error::other)?;
        pixels.resize(1024 * 1024 * 3, 128);
        let file = std::fs::File::create(&path)?;
        let mut encoder = png::Encoder::new(file, 1024, 1024);
        encoder.set_color(png::ColorType::Rgb);
        let mut writer = encoder.write_header().map_err(std::io::Error::other)?;
        writer.write_image_data(&pixels).map_err(std::io::Error::other)?;
        writer.finish().map_err(std::io::Error::other)?;
        Ok::<_, WorkError>(ImageSource::file(path))
    })?;
    let mut source =
        Some(Session::terminal(fixture.wake.completion(&fixture.client, &mut job)?.outcome)?);
    let deadline = fixture.session.preview_now().checked_add(Duration::from_secs(30))?;
    let token =
        fixture.session.offer_preview(&mut source, deadline)?.token().ok_or("Preview refused")?;
    let target = fixture.offered_preview(token)?;
    fixture.session.did_present_cpu(target)?;
    let camera = fixture.session.accepted_camera().ok_or("Preview camera absent")?;
    assert_eq!(camera.extent, Extent::new(1024, 1024)?);
    assert_eq!(camera.viewport.surface(), Extent::new(8, 8)?);
    assert_eq!(camera.viewport.zoom(), 1.0 / 128.0);
    assert!(fixture.session.preview_active());
    let _directory = fixture.finish()?;
    Ok(())
}
