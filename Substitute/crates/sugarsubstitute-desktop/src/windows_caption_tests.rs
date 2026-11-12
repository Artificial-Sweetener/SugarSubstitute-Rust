// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native pixels compared with independently rendered hidden Qt6.11.2 reference buttons.
use super::*;
use windows::Win32::Graphics::Gdi::{CreateCompatibleDC, DeleteDC};

// Full BGRA FNV-1a digests of QWidget::render output; no desktop capture or repaint involved.
// Original close SVG SHA256 dd1f11891350658cdb76b10199161aebebc9b6fdf70b08fb3239f3c7d133a9f1.
const REFERENCES: [[u64; 24]; 9] = [
    [
        0x23e7be10c46c47a8,
        0x0f63e21eec568066,
        0x3ec075d1f7bd0031,
        0x320aa8372a246085,
        0x8188f060f6aecde5,
        0xc8d80cb4456acaa5,
        0x690b033d8886eca8,
        0xc7081be4b3f72846,
        0x517de06632232731,
        0x7e38b0762af0c0fe,
        0xdea475928d25727c,
        0x191993671b148281,
        0x99be550a361a1619,
        0x6cd02b0cdc230009,
        0x115fae7b82cb692d,
        0xf6c21c699a4f0d85,
        0x954907fd0da068c5,
        0x38d9d9cdfa244b25,
        0x5a3d4cff007da7f9,
        0x2161200a32980ee9,
        0x48b8457dd569838d,
        0x39635000733f4061,
        0xdea475928d25727c,
        0x191993671b148281,
    ],
    [
        0xb1faa827cd3cf8f5,
        0xf89437beabe86d35,
        0x181c66770959abe5,
        0xb53ca5318875afe5,
        0xac8a04ab5c312fe5,
        0x56380adf51cab4a5,
        0x4cb8c1e3efaef298,
        0x6927e70e95c40ef6,
        0xdc32326ab426eef1,
        0xcb367f045fe563a4,
        0xe247358bd9234fbb,
        0xfa4ae1988f0c9971,
        0xb6a3477a5814e86d,
        0x6e6354d045aba48d,
        0xe092d4aa42c106f5,
        0x99f5b63f388e0f65,
        0x5ddd278ca5dad565,
        0x22a73e6101cc80a5,
        0x92ef0c3c0cf26889,
        0xac08514137b04359,
        0x056ecb14426f566d,
        0x2ceb52b0d6fadfd9,
        0xe247358bd9234fbb,
        0xfa4ae1988f0c9971,
    ],
    [
        0x5977ad21ca2efda5,
        0xf4a6b8d38a320da5,
        0x0c4a0251ab24b1a5,
        0xf90ad02e0794a085,
        0x17bf07280f8205a5,
        0x1ae6c68cd3af81a5,
        0x54c5de358b356098,
        0xc45a72704d951676,
        0x7106da00b0a34971,
        0xdb8bdd35cb133865,
        0x8614b1d1f99795dc,
        0x88552ded91515d46,
        0xc57f83ce876197e5,
        0x297cd44160a771e5,
        0x25b9312a014d2165,
        0x32d82850673e54b5,
        0x26068ebefc9cb635,
        0xf54b262d9c2e29a5,
        0xda34fb049d5db791,
        0x3bd82caaa60178e1,
        0x2e9c89afb2cbce7d,
        0xf588cfb38bd11bb5,
        0x8614b1d1f99795dc,
        0x88552ded91515d46,
    ],
    [
        0x42d707fab25f3798,
        0xdf2264988bc6e5d6,
        0xe91fe0548368a931,
        0x475d67e5b14b0d05,
        0xee463d7cdfe39165,
        0xb05089b2514eabe5,
        0x80871f48cdf3c0e8,
        0xb33e279e6f00d806,
        0xbed5d0280fc26471,
        0xbba9be99edae8423,
        0xad60d6ad02c9b13a,
        0xc506b7df139e0599,
        0x89995c13500b2a39,
        0xf01301cb5913fb49,
        0x900e4e0e24f1b06d,
        0x5794ecd275129115,
        0x34a3b9b9c5fc4915,
        0x926982cc32077ea5,
        0x9db48ec8d92572a1,
        0x4f5e1d0489017991,
        0x38006fb040dba55d,
        0xe902992981fb0245,
        0xad60d6ad02c9b13a,
        0xc506b7df139e0599,
    ],
    [
        0xed4d827ce3cb7378,
        0xca61a9f4f5edff96,
        0x998864757d75ed71,
        0x8a9e14941afd28e5,
        0xbc45c72c8dc44665,
        0xb8ea58e6cad5a3a5,
        0x1f2c33fcf903bf78,
        0xecceba568c679216,
        0x7b6c78dc7089fb71,
        0x1ad5d09f273b693b,
        0x6dcc2d89daad9d96,
        0xdabc9fba2072992b,
        0x64863a72bd743071,
        0x44fee4a9e268a2c1,
        0x7cd8ffc050f74c1d,
        0x13b6354a98f4afe5,
        0x34b5d1eb933af9e5,
        0xdddcd8f95d6001e5,
        0x9d01f7c8f2874169,
        0x51f3bba17f766979,
        0xa49526181271eb4d,
        0x9c197594023d2285,
        0x6dcc2d89daad9d96,
        0xdabc9fba2072992b,
    ],
    [
        0x886e562f0225ee15,
        0x0fb4a9f988c79bf5,
        0xe7b37ac85fc1ad65,
        0xd05c3f8aed89cb85,
        0x78e1ebac56be6425,
        0x587e540b0d2606a5,
        0xc28c06dd426146a8,
        0xa5ac7c2574718746,
        0xc638d32640826e31,
        0xe46da0faa9102848,
        0x96880aa1ffb07913,
        0x7771b81d64a34f2a,
        0xb25c9984405551bd,
        0x725158710151135d,
        0xb23fdb07f4f77a95,
        0xce9ed0647f620315,
        0xfc14b5a93e16ee15,
        0x936871c95f813f25,
        0x0828e56be9150981,
        0xfb60888e34f92cf1,
        0x0b4c999fb3338b7d,
        0x42b5229de8be2069,
        0x96880aa1ffb07913,
        0x7771b81d64a34f2a,
    ],
    [
        0xc02113c56bd8fd48,
        0xd933f72dedf59a06,
        0xc42735f92563ad31,
        0x5a8e7c5a26a88645,
        0xe6095ad743b1d3a5,
        0xea032cb327f445a5,
        0xe9559af0867e3e48,
        0x54565a9e104bf1e6,
        0x83cb0916935677b1,
        0x265c13f358a66ddf,
        0xdc16909884384c6e,
        0x173e06b5d42c9b05,
        0x04f848766ac165c9,
        0x04f2d9a2893d3279,
        0xf9742cbf17e19cad,
        0xc0ad3c332685a045,
        0xeedb4486d72bb685,
        0xeffedd9e9c5a50e5,
        0x7f0e869094702dd9,
        0x1a2a781bd61b9049,
        0x88655bb4525407ad,
        0xea5caabfe3218efd,
        0xdc16909884384c6e,
        0x173e06b5d42c9b05,
    ],
    [
        0x1335e83883837685,
        0xd123de3fee08f185,
        0x1d3e1f0d1b3fbaa5,
        0xf58c4f1a285e8c05,
        0x34fec8559a4dc9e5,
        0x269344aee6a71a25,
        0x89e373e2989bcdb8,
        0x600e6f244e5278b6,
        0x92228bbcd41a20f1,
        0x3e2baea25fdb3e19,
        0x87894ef7a3f22696,
        0x98b4f273d86a7e27,
        0xf59728c214562095,
        0xc39059ac0e82df55,
        0x436a7d91150adce5,
        0xdbffcc9b708d2d75,
        0x82df7ab4912bb675,
        0x7c42d9cef6bb6e25,
        0xb272998d4632d971,
        0x9761a8a1aa324381,
        0xf295850d9fc8ae1d,
        0xf0aaacd74b35b97d,
        0x87894ef7a3f22696,
        0x98b4f273d86a7e27,
    ],
    [
        0x8906d7d939515918,
        0x228ed5ee9d1e8336,
        0xcafaf331d335b871,
        0x9adfbe329957baa5,
        0x8b934a8a77c94fa5,
        0x1d9bc3dee05d3ca5,
        0x8dfbceffa4ba1f18,
        0x1d22603c0d808c36,
        0x56f7188a88f7e6f1,
        0x7d4c444c66002429,
        0xe2660b4c1c099cb4,
        0xf985f3c0ba78944f,
        0xe6f4571a6a6f7821,
        0xb5b3e86115e41d31,
        0x9b2c620f50d9b11d,
        0x4552ac8a426772a5,
        0x84952d4e75d61ea5,
        0x81d5428ec2da73a5,
        0x91dc346a2135e349,
        0x29af629d86d0c759,
        0x89a5f6ac32e8002d,
        0xf2962ae49beac36d,
        0xe2660b4c1c099cb4,
        0xf985f3c0ba78944f,
    ],
];

/// Cover every pixel of all four original shapes, both themes and three interaction states.
#[test]
fn native_caption_pixels_match_original_qt_vectors_and_feedback()
-> Result<(), Box<dyn std::error::Error>> {
    // SAFETY: An owned memory DC displays no window and cannot interact with the user's desktop.
    let dc = unsafe { CreateCompatibleDC(None) };
    assert!(!dc.is_invalid());
    let result = (|| {
        for (row, dpi) in [96, 120, 144, 168, 192, 240, 288, 336, 384].into_iter().enumerate() {
            let rect = RECT {
                left: 0,
                top: 0,
                right: ((46 * dpi + 48) / 96) as i32,
                bottom: ((32 * dpi + 48) / 96) as i32,
            };
            let mut column = 0;
            for dark in [false, true] {
                for (id, maximized) in [(1, false), (2, false), (2, true), (3, false)] {
                    for (hot, pressed) in [(false, false), (true, false), (true, true)] {
                        buffered(dc, rect, |_target, buffer| {
                            background(
                                buffer,
                                rect,
                                Caption {
                                    dark,
                                    material: NativeMaterial::Mica,
                                    update: Default::default(),
                                },
                                hot,
                                pressed,
                                id == 3,
                                dpi,
                            )?;
                            pixels::with_pixels(buffer, rect, |pixels| {
                                glyphs::draw(
                                    pixels,
                                    id,
                                    maximized,
                                    dpi,
                                    if dark || id == 3 && (hot || pressed) { 255 } else { 0 },
                                )?;
                                assert_eq!(
                                    pixels.digest(),
                                    REFERENCES[row][column],
                                    "dpi={dpi} dark={dark} id={id} maximized={maximized} hot={hot} pressed={pressed}"
                                );
                                Ok(())
                            })
                        })?;
                        column += 1;
                    }
                }
            }
        }
        // At125%, independent child-edge rounding yields a57px peer for a46logical-pixel button.
        let rect = RECT { left: 0, top: 0, right: 57, bottom: 40 };
        for ((hot, pressed), expected) in [(false, false), (true, false), (true, true)]
            .into_iter()
            .zip([0x0700326183c733d9, 0xd1260c9163f723f3, 0x0887276925b601b1])
        {
            buffered(dc, rect, |_, buffer| {
                background(
                    buffer,
                    rect,
                    Caption {
                        dark: true,
                        material: NativeMaterial::Mica,
                        update: Default::default(),
                    },
                    hot,
                    pressed,
                    true,
                    120,
                )?;
                pixels::with_pixels(buffer, rect, |pixels| {
                    glyphs::draw(pixels, 3, false, 120, 255)?;
                    assert_eq!(pixels.digest(), expected, "fractional native peer bounds");
                    Ok(())
                })
            })?;
        }
        Ok::<(), DesktopError>(())
    })();
    // SAFETY: Every native buffer has retired before its original memory DC is released.
    assert!(unsafe { DeleteDC(dc) }.as_bool());
    result.map_err(Into::into)
}

/// The update glyph is drawn wholly in its tint, ready and warning differ, and both themes keep
/// it inside the caption's glyph box at every supported scale.
#[test]
fn update_glyph_is_tinted_for_ready_and_warning() -> Result<(), Box<dyn std::error::Error>> {
    // SAFETY: An owned memory DC displays no window and cannot interact with the user's desktop.
    let dc = unsafe { CreateCompatibleDC(None) };
    assert!(!dc.is_invalid());
    let result = (|| {
        for dpi in [96, 120, 144, 192, 288] {
            let rect = RECT {
                left: 0,
                top: 0,
                right: ((46 * dpi + 48) / 96) as i32,
                bottom: ((32 * dpi + 48) / 96) as i32,
            };
            for dark in [false, true] {
                let mut inks = Vec::new();
                for warning in [false, true] {
                    let tint = glyphs::update_tint(warning, dark);
                    buffered(dc, rect, |_target, buffer| {
                        pixels::with_pixels(buffer, rect, |pixels| {
                            pixels.fill([0; 4]);
                            glyphs::update(pixels, dpi, tint);
                            let (count, box_) = pixels.ink(tint);
                            let scale = |v: usize| v * dpi as usize / 96;
                            assert!(count > scale(20), "dpi={dpi} dark={dark}: {count} pixels");
                            assert!(box_[0] >= scale(18) && box_[2] <= scale(29), "{box_:?}");
                            assert!(box_[1] >= scale(11) && box_[3] <= scale(22), "{box_:?}");
                            inks.push(count);
                            Ok(())
                        })
                    })?;
                }
                assert_ne!(glyphs::update_tint(false, dark), glyphs::update_tint(true, dark));
                assert_eq!(inks[0], inks[1], "the tint changes only the color");
            }
        }
        Ok::<(), DesktopError>(())
    })();
    // SAFETY: Every native buffer has retired before its original memory DC is released.
    assert!(unsafe { DeleteDC(dc) }.as_bool());
    result.map_err(Into::into)
}
