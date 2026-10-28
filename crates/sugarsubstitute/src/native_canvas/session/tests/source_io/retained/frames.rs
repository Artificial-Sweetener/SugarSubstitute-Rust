// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise selected output replacement through the real CPU rendering session.

use super::*;

impl Fixture {
    /// Stop submitting image work and observe every existing document job before transfer.
    fn quiesce_document(&mut self) -> Result<(), Box<dyn Error>> {
        self.session.input(Input::Drawable(false))?;
        self.clock.0.fetch_add(1_000_000_000, Ordering::SeqCst);
        loop {
            let observed = self.wake.revision();
            self.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
            self.session.poll(Duration::ZERO)?;
            if self.session.failed.is_some() {
                return Err("Canvas failed while settling document work".into());
            }
            if self.session.rendering.is_none()
                && self.session.displaying.is_none()
                && self.session.pending_retirement.is_none()
                && self.session.deferred_retirement.is_none()
                && self.session.retirement.is_none()
            {
                return Ok(());
            }
            self.wake.wait_after(observed).map_err(|cause| {
                format!(
                    "Document settlement (rendering={}, displaying={}, cleanup={}): {cause}",
                    self.session.rendering.is_some(),
                    self.session.displaying.is_some(),
                    self.session.retirement.is_some()
                )
            })?;
        }
    }

    /// Observe a complete pending CPU frame while the shared executor delivers real work.
    pub(super) fn pending_frame(&mut self) -> Result<(FrameStamp, Extent, u32), Box<dyn Error>> {
        loop {
            let observed = self.wake.revision();
            self.client.drain_completions(std::num::NonZeroUsize::MIN.saturating_add(31))?;
            self.session.poll(Duration::ZERO)?;
            if self.session.failed.is_some() {
                return Err("Canvas failed while preparing selected output".into());
            }
            if let Some(frame) = self.session.frame().filter(|frame| frame.pending) {
                let extent = frame.damage.dirty().surface();
                let center = u64::from(extent.height() / 2) * u64::from(extent.width())
                    + u64::from(extent.width() / 2);
                let pixel = *frame.pixels.get(usize::try_from(center)?).ok_or("Center absent")?;
                return Ok((frame.target, extent, pixel));
            }
            self.wake.wait_after(observed)?;
        }
    }
}

#[test]
fn withdrawn_document_keeps_its_pixels_and_camera_while_the_shared_session_prepares_another()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    fixture.session.resize(Extent::new(8, 8)?)?;
    fixture.session.input(Input::Drawable(true))?;
    let mut red = Some(fixture.encoded([255, 0, 0])?);
    let mut blue = Some(fixture.encoded([0, 0, 255])?);
    assert!(fixture.session.offer_source(&mut red, CacheReuse::Compatible, None)?);
    fixture.clock.0.store(1_000_000_000, Ordering::SeqCst);
    let (red_frame, _, red_pixel) = fixture.pending_frame()?;
    assert!(fixture.session.withdraw_document().is_err());
    assert_eq!(fixture.session.frame().ok_or("Pending red frame lost")?.target, red_frame);
    fixture.session.did_present_cpu(red_frame)?;
    fixture.quiesce_document()?;
    let view = fixture.session.document.tools.navigation().viewport();
    let pixels = fixture.session.frame().ok_or("Accepted red frame lost")?.pixels.as_ptr();
    let retained = fixture.session.withdraw_document()?;
    assert!(fixture.session.frame().is_none());
    assert!(fixture.session.document.scene.is_none());
    assert!(fixture.session.resources.is_some());
    assert!(fixture.session.runtime.is_some());

    assert!(fixture.session.offer_source(&mut blue, CacheReuse::Compatible, None)?);
    fixture.session.input(Input::Drawable(true))?;
    fixture.clock.0.store(3_000_000_000, Ordering::SeqCst);
    let (blue_frame, _, blue_pixel) = fixture.pending_frame()?;
    assert!(blue_frame.generation > red_frame.generation);
    assert!((blue_pixel >> 16) & 0xff < 16 && blue_pixel & 0xff > 224);
    let frame = retained.cpu_frame().ok_or("Withdrawn projection lost")?;
    assert!(!frame.pending);
    assert_eq!(frame.target, red_frame);
    assert_eq!(frame.pixels.as_ptr(), pixels);
    assert_eq!(frame.pixels[36], red_pixel);
    assert_eq!(retained.tools.navigation().viewport(), view);
    assert!(retained.scene.as_ref().is_some_and(|scene| scene.source.is_some()));
    fixture.session.did_present_cpu(blue_frame)?;
    fixture.session.retire(Retired { _document: Some(retained), ..Default::default() })?;
    let _directory = fixture.finish()?;
    Ok(())
}

#[test]
fn newer_output_and_resize_replace_unpresented_pixels_on_the_shared_executor()
-> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    fixture.session.resize(Extent::new(8, 8)?)?;
    fixture.session.input(Input::Drawable(true))?;
    let mut red = Some(fixture.encoded([255, 0, 0])?);
    let mut blue = Some(fixture.encoded([0, 0, 255])?);
    assert!(fixture.session.offer_source(&mut red, CacheReuse::Compatible, None)?);
    fixture.clock.0.store(1_000_000_000, Ordering::SeqCst);
    let (red_frame, extent, red_pixel) = fixture.pending_frame()?;
    assert_eq!(extent, Extent::new(8, 8)?);
    assert!((red_pixel >> 16) & 0xff > 224 && red_pixel & 0xff < 16);

    assert!(fixture.session.offer_source(&mut blue, CacheReuse::Compatible, None)?);
    assert!(fixture.session.frame().is_none());
    assert!(fixture.session.did_present_cpu(red_frame).is_err());
    fixture.clock.0.store(2_000_000_000, Ordering::SeqCst);
    let (blue_frame, extent, blue_pixel) = fixture.pending_frame()?;
    assert_eq!(extent, Extent::new(8, 8)?);
    assert!((blue_pixel >> 16) & 0xff < 16 && blue_pixel & 0xff > 224);

    fixture.session.resize(Extent::new(10, 8)?)?;
    assert!(fixture.session.frame().is_none());
    assert!(fixture.session.did_present_cpu(blue_frame).is_err());
    fixture.clock.0.store(3_000_000_000, Ordering::SeqCst);
    let (resized_frame, extent, pixel) = fixture.pending_frame()?;
    assert_eq!(extent, Extent::new(10, 8)?);
    assert!((pixel >> 16) & 0xff < 16 && pixel & 0xff > 224);
    fixture.session.did_present_cpu(resized_frame)?;
    assert!(!fixture.session.frame().ok_or("Accepted output disappeared")?.pending);
    let _directory = fixture.finish()?;
    Ok(())
}

#[cfg(feature = "native-composition")]
#[test]
fn embedded_selection_never_reuses_an_acknowledged_older_output() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::start(true)?;
    fixture.session.resize(Extent::new(8, 8)?)?;
    fixture.session.input(Input::Drawable(true))?;
    let mut red = Some(fixture.encoded([255, 0, 0])?);
    let mut blue = Some(fixture.encoded([0, 0, 255])?);
    assert!(fixture.session.offer_source(&mut red, CacheReuse::Compatible, None)?);
    fixture.clock.0.store(1_000_000_000, Ordering::SeqCst);
    let (red_frame, _, _) = fixture.pending_frame()?;
    fixture.session.did_present_cpu(red_frame)?;
    assert_eq!(
        fixture.session.selected_cpu_frame().ok_or("Current red frame absent")?.0.target,
        red_frame
    );

    assert!(fixture.session.offer_source(&mut blue, CacheReuse::Compatible, None)?);
    assert_eq!(fixture.session.frame().ok_or("Retained red frame absent")?.target, red_frame);
    assert!(fixture.session.selected_cpu_frame().is_none());
    fixture.clock.0.store(2_000_000_000, Ordering::SeqCst);
    let (blue_frame, _, pixel) = fixture.pending_frame()?;
    assert!((pixel >> 16) & 0xff < 16 && pixel & 0xff > 224);
    assert_eq!(
        fixture.session.selected_cpu_frame().ok_or("Current blue frame absent")?.0.target,
        blue_frame
    );
    fixture.session.did_present_cpu(blue_frame)?;
    fixture.session.resize(Extent::new(10, 8)?)?;
    assert!(fixture.session.selected_cpu_frame().is_none());
    fixture.clock.0.store(3_000_000_000, Ordering::SeqCst);
    let (resized, extent, _) = fixture.pending_frame()?;
    assert_eq!(extent, Extent::new(10, 8)?);
    assert_eq!(
        fixture.session.selected_cpu_frame().ok_or("Resized frame absent")?.0.target,
        resized
    );
    fixture.session.did_present_cpu(resized)?;
    let _directory = fixture.finish()?;
    Ok(())
}
