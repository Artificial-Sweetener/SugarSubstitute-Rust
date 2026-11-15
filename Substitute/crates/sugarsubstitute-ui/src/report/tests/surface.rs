// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove that every report paints one opaque dialog surface at every display scale.
use super::*;

/// The waiting line and the question are the same dialog: at every display scale both paint an
/// opaque surface over the whole window, so the window's material never shows behind the body,
/// and the footer band runs from the left edge to the right edge.
#[test]
fn waiting_and_question_paint_the_same_opaque_dialog_at_every_scale() -> Result {
    let mut resources = SceneResources::new("en")?;
    let kind = ReportKind::UnresponsiveCopy;
    let waiting = "Finishing closing the previous window…";
    let question = "The previous SugarSubstitute window isn't responding. Close it and start? \
                    Anything it hadn't saved may be lost.";
    for scale in [1.0_f32, 1.5, 1.75] {
        let mut frames = Vec::new();
        for (message, close) in [(waiting, false), (question, true)] {
            let content = ReportContent {
                kind,
                severity: ReportSeverity::Info,
                window_title: "SugarSubstitute".into(),
                title: "Starting SugarSubstitute".into(),
                message: message.into(),
                summary: Vec::new(),
                has_details: false,
            };
            let mut state = ReportState::new(kind);
            state.set(ReportAction::Close, CommandState { visible: true, enabled: close });
            let width = (WIDTH * scale).round() as u32;
            let measure =
                PaintSpec { theme: Theme::Dark, scale, width, height: scale.ceil() as u32 };
            let measured = ReportLayout::measure(
                &content,
                &LABELS,
                &state,
                measure,
                &mut resources,
                None,
                4000.0,
            )?;
            let height = (measured.height(0.0) * scale).ceil() as u32;
            let spec = PaintSpec { theme: Theme::Dark, scale, width, height };
            let frame = measured.compose(&content, &LABELS, &state, spec, None)?;
            frames.push((paint(frame, spec, &mut resources)?, spec));
        }
        for (pixels, spec) in &frames {
            let at = |x: u32, y: u32| -> [u8; 4] {
                let index = ((y * spec.width + x) * 4) as usize;
                let p = &pixels.pixels()[index..index + 4];
                [p[0], p[1], p[2], p[3]]
            };
            let translucent = (0..spec.height)
                .flat_map(|y| (0..spec.width).map(move |x| (x, y)))
                .filter(|(x, y)| at(*x, *y)[3] != 255)
                .count();
            assert_eq!(translucent, 0, "every pixel is opaque at scale {scale}");
            let footer_top = (0..spec.height)
                .rev()
                .find(|y| at(spec.width / 2, *y) != [32, 32, 32, 255])
                .ok_or("footer band absent")?;
            for x in [0, 1, spec.width / 2, spec.width - 1] {
                assert_eq!(at(x, spec.height - 1), [32, 32, 32, 255], "footer at x {x}, {scale}");
            }
            let title_bar = (super::super::layout::TITLE_BAR * scale).ceil() as u32;
            for y in [title_bar + 2, (title_bar + footer_top) / 2, footer_top - 2] {
                for x in [0, 1, spec.width - 1] {
                    assert_eq!(at(x, y), [43, 43, 43, 255], "dialog surface at {x},{y}, {scale}");
                }
            }
        }
        // Apart from their text and buttons, both frames paint the same edges: the dialog
        // surface down to a footer band of the same height, its top border antialiased only
        // where the band's edge falls between pixels.
        let edges = |(pixels, spec): &(PreparedFrame, PaintSpec)| -> (Vec<[u8; 4]>, u32) {
            let mut colors: Vec<[u8; 4]> = Vec::new();
            let mut band = 0;
            for y in 0..spec.height {
                for x in [0, spec.width - 1] {
                    let index = ((y * spec.width + x) * 4) as usize;
                    let p = &pixels.pixels()[index..index + 4];
                    let color = [p[0], p[1], p[2], p[3]];
                    band += u32::from(x == 0 && color == [32, 32, 32, 255]);
                    if !colors.contains(&color) {
                        colors.push(color);
                    }
                }
            }
            (colors, band)
        };
        let (waiting, question) = (edges(&frames[0]), edges(&frames[1]));
        for (colors, _) in [&waiting, &question] {
            for color in colors {
                let border =
                    color[3] == 255 && color[0] == color[1] && (29..=32).contains(&color[0]);
                assert!(*color == [43, 43, 43, 255] || border, "edge {color:?} at {scale}");
            }
        }
        assert!(waiting.1.abs_diff(question.1) <= 1, "footer bands {waiting:?} {question:?}");
    }
    Ok(())
}
