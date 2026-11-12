// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Rendered editing cursors become exact native cursors, cached per DPI with bounded custody.

use super::{
    tests::{LARGE, STANDARD, bitmap_pixels, icon_info, memory_dc},
    *,
};
use sugarsubstitute_canvas::{AffineCursor, BrushOutline};
use sugarsubstitute_ui::cursors::{CursorArt, PreciseMark};

fn ring(diameter: u32) -> CursorIntent {
    CursorIntent::BrushOutline(BrushOutline::new(diameter, [40, 160, 255], false))
}

#[test]
fn rendered_cursors_keep_their_artwork_pixels_and_hotspot_natively() -> Result<(), Box<dyn Error>> {
    let dc = memory_dc()?;
    for (art, scale) in [
        (CursorArt::BrushOutline { diameter: 60, color: [255, 0, 128], erase: true }, 1.0),
        (CursorArt::Precise(PreciseMark::Add), 1.5),
        (CursorArt::SelectionTranslate, 2.0),
        (CursorArt::MoveCut, 1.0),
    ] {
        let image = art.render(scale)?;
        let cursor = resource::rendered_cursor(&image)?;
        let info = icon_info(&cursor)?;
        assert!(!info.0.fIcon.as_bool());
        assert_eq!([info.0.xHotspot, info.0.yHotspot], image.hotspot(), "{art:?}");
        assert_eq!(image.width(), image.height());
        let pixels = bitmap_pixels(&info.0, image.width() as i32, &dc)?;
        let expected: Vec<u8> = image
            .rgba()
            .chunks_exact(4)
            .flat_map(|pixel| [pixel[2], pixel[1], pixel[0], pixel[3]])
            .collect();
        assert_eq!(pixels, expected, "{art:?} keeps straight-alpha pixels");
    }
    Ok(())
}

#[test]
fn the_largest_brush_ring_still_becomes_one_native_cursor() -> Result<(), Box<dyn Error>> {
    let image = CursorArt::BrushOutline {
        diameter: BrushOutline::MAX_DIAMETER,
        color: [1, 2, 3],
        erase: false,
    }
    .render(2.0)?;
    let cursor = resource::rendered_cursor(&image)?;
    let info = icon_info(&cursor)?;
    assert_eq!([info.0.xHotspot, info.0.yHotspot], image.hotspot());
    Ok(())
}

#[test]
fn editing_intents_share_artwork_and_reuse_one_native_cursor_per_dpi() -> Result<(), Box<dyn Error>>
{
    let cursors = Cursors::sized(STANDARD)?;
    let translate = cursors.resolve(STANDARD, CursorIntent::SelectionTranslate)?;
    assert_eq!(cursors.resolve(STANDARD, CursorIntent::Move)?.handle(), translate.handle());
    let distinct = [
        CursorIntent::Precise,
        CursorIntent::PreciseAdd,
        CursorIntent::PreciseSubtract,
        CursorIntent::MoveCut,
        ring(30),
    ]
    .map(|intent| cursors.resolve(STANDARD, intent).map(|cursor| cursor.handle()));
    let mut handles = vec![translate.handle()];
    for handle in distinct {
        let handle = handle?;
        assert!(!handles.contains(&handle));
        handles.push(handle);
    }
    assert_eq!(cursors.resolve(STANDARD, ring(30))?.handle(), handles[5]);
    let scaled = cursors.resolve(LARGE, CursorIntent::SelectionTranslate)?;
    assert_ne!(scaled.handle(), translate.handle(), "a new DPI renders new artwork");
    assert_eq!(cursors.0.borrow().rendered.len(), 1, "other DPIs leave the cache");
    assert!(translate.owned().is_some(), "a displaced cursor lives while still held");
    assert_eq!(cursors.resolve(LARGE, CursorIntent::Arrow)?.handle(), cursors.arrow());
    let forbidden = cursors.resolve(LARGE, CursorIntent::Forbidden)?;
    assert!(forbidden.owned().is_none(), "the forbidden cursor is the shared system one");
    assert!(![cursors.arrow(), translate.handle()].contains(&forbidden.handle()));
    Ok(())
}

#[test]
fn zooming_brush_rings_stay_bounded_and_retirement_releases_them() -> Result<(), Box<dyn Error>> {
    let cursors = Cursors::sized(STANDARD)?;
    let first = cursors.resolve(STANDARD, ring(10))?;
    for diameter in 11..30 {
        cursors.resolve(STANDARD, ring(diameter))?;
        assert!(cursors.0.borrow().rendered.len() <= 8);
    }
    let held = first.owned().ok_or("rendered cursors are private")?;
    assert_eq!(Rc::strong_count(&held), 2, "only this test still holds the evicted ring");
    let again = cursors.resolve(STANDARD, ring(10))?;
    assert_ne!(again.handle(), first.handle(), "an evicted ring renders again");
    cursors.retire_custom();
    assert_eq!(cursors.0.borrow().rendered.len(), 0);
    assert_eq!(again.owned().map(|cursor| Rc::strong_count(&cursor)), Some(2));
    Ok(())
}

#[test]
fn large_rings_stay_within_the_cached_byte_budget() -> Result<(), Box<dyn Error>> {
    let cursors = Cursors::sized(STANDARD)?;
    for diameter in [1024, 1280, 1536, 1792, 2048] {
        cursors.resolve(STANDARD, ring(diameter))?;
        assert!(
            cursors.0.borrow().rendered.bytes() <= 32 << 20
                || cursors.0.borrow().rendered.len() == 1
        );
    }
    let latest = cursors.resolve(STANDARD, ring(2048))?;
    assert_eq!(cursors.resolve(STANDARD, ring(2048))?.handle(), latest.handle());
    Ok(())
}

#[test]
fn shared_edges_use_system_cursors_and_transform_cursors_stay_bounded() -> Result<(), Box<dyn Error>>
{
    let cursors = Cursors::sized(STANDARD)?;
    let system = [
        ResizeDirection::Horizontal,
        ResizeDirection::Vertical,
        ResizeDirection::DiagonalDown,
        ResizeDirection::DiagonalUp,
    ]
    .map(|direction| cursors.resolve(STANDARD, CursorIntent::Resize(direction)));
    let forbidden = cursors.resolve(STANDARD, CursorIntent::Forbidden)?;
    let mut handles = vec![cursors.arrow(), forbidden.handle()];
    for resource in system {
        let resource = resource?;
        assert!(resource.owned().is_none(), "system cursors are shared");
        assert!(!handles.contains(&resource.handle()));
        handles.push(resource.handle());
    }
    let level = cursors.resolve(STANDARD, CursorIntent::Affine(AffineCursor::resize(0.4)))?;
    let same = cursors.resolve(STANDARD, CursorIntent::Affine(AffineCursor::resize(180.0)))?;
    assert_eq!(level.handle(), same.handle(), "opposite directions share one bucket");
    let rotate = cursors.resolve(STANDARD, CursorIntent::Affine(AffineCursor::rotate(0.0)))?;
    assert_ne!(rotate.handle(), level.handle());
    for degrees in 1..40 {
        cursors
            .resolve(STANDARD, CursorIntent::Affine(AffineCursor::resize(f64::from(degrees))))?;
    }
    assert!(cursors.0.borrow().rendered.len() <= 8, "turned cursors stay within the cache");
    Ok(())
}
