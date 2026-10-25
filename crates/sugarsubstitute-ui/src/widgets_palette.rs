// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Product form colors and state precedence, kept outside surface layout code.
use super::{ControlState, Decoration, Palette};

impl Decoration {
    pub(super) fn palette(self) -> Palette {
        let white = |alpha| [255, 255, 255, alpha];
        let uniform = |radius, fill, border| Palette {
            radius,
            fill,
            border,
            top: border,
            bottom: border,
            insets: if border[3] == 0 { [0.0; 4] } else { [1.0; 4] },
        };
        match self {
            Self::Button { primary, state } => {
                let fill = if primary {
                    match state {
                        ControlState::Disabled => [52, 52, 52, 255],
                        ControlState::Hover | ControlState::Pressed => [243, 58, 128, 255],
                        _ => [255, 64, 137, 255],
                    }
                } else {
                    white(match state {
                        ControlState::Rest => 15,
                        ControlState::Hover => 21,
                        ControlState::Pressed => 8,
                        ControlState::Disabled => 11,
                    })
                };
                let mut palette = uniform(5.0, fill, white(if primary { 20 } else { 14 }));
                palette.top = white(20);
                palette
            }
            Self::Field { selection, focused, state } => {
                let fill = white(if focused && !selection {
                    10
                } else if state == ControlState::Hover {
                    21
                } else {
                    15
                });
                let mut palette = uniform(5.0, fill, white(if selection { 14 } else { 20 }));
                palette.top = white(20);
                if !selection {
                    palette.bottom = if focused { [255, 64, 137, 255] } else { white(139) };
                    if focused {
                        palette.insets[3] = 2.0;
                    }
                }
                palette
            }
            Self::Section => uniform(22.0, white(10), white(19)),
            Self::ProgressSection => uniform(18.0, white(10), white(19)),
            Self::IconBadge => uniform(14.0, [255, 64, 137, 31], [255, 64, 137, 61]),
            Self::Choice { state } => {
                uniform(18.0, white(if state == ControlState::Hover { 6 } else { 0 }), white(0))
            }
            Self::Summary => uniform(18.0, white(6), white(0)),
            Self::Output => uniform(5.0, [16, 16, 16, 255], white(37)),
            Self::Radio { checked } => {
                let mut palette =
                    uniform(10.0, white(0), if checked { [255, 64, 137, 255] } else { white(153) });
                if checked {
                    palette.insets = [5.0; 4];
                }
                palette
            }
            Self::Check { checked } => uniform(
                2.0,
                if checked { [255, 64, 137, 255] } else { [59, 59, 59, 255] },
                if checked { [255, 64, 137, 255] } else { [133, 133, 133, 255] },
            ),
            Self::Close { state } => uniform(
                0.0,
                if matches!(state, ControlState::Hover | ControlState::Pressed) {
                    [196, 43, 28, 255]
                } else {
                    white(0)
                },
                white(0),
            ),
            Self::Body => Palette {
                radius: 0.0,
                fill: [32, 32, 32, 150],
                border: white(0),
                top: white(18),
                bottom: white(0),
                insets: [0.0, 1.0, 0.0, 0.0],
            },
        }
    }
}
