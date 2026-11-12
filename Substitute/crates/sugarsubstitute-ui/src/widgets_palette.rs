// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Product form colors and state precedence, kept outside surface layout code.
use super::{ControlState, Decoration, Palette};
use crate::accent::AccentPalette;
use sugarsubstitute_contracts::configuration::Theme;

impl Decoration {
    /// Resolve neutral and accent roles for the mode retained by the prepared surface.
    pub(super) fn palette(self, theme: Theme, accent: AccentPalette) -> Palette {
        let dark = theme == Theme::Dark;
        let shades = accent.variants(dark);
        let white = |alpha| [255, 255, 255, alpha];
        let neutral = |alpha| if dark { white(alpha) } else { [0, 0, 0, alpha] };
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
                        ControlState::Disabled => {
                            if dark {
                                [52, 52, 52, 255]
                            } else {
                                [205, 205, 205, 255]
                            }
                        }
                        _ => shades.button(state),
                    }
                } else if dark {
                    white(match state {
                        ControlState::Rest => 15,
                        ControlState::Hover => 21,
                        ControlState::Pressed => 8,
                        ControlState::Disabled => 11,
                    })
                } else {
                    match state {
                        ControlState::Rest => white(179),
                        ControlState::Hover => [249, 249, 249, 128],
                        _ => [249, 249, 249, 76],
                    }
                };
                let mut palette = uniform(
                    5.0,
                    fill,
                    neutral(if dark { if primary { 20 } else { 14 } } else { 19 }),
                );
                palette.top = neutral(20);
                if !dark && !primary {
                    palette.bottom =
                        neutral(if matches!(state, ControlState::Rest | ControlState::Hover) {
                            47
                        } else {
                            19
                        });
                }
                if primary && state != ControlState::Disabled {
                    palette.border = match (dark, state) {
                        (true, ControlState::Pressed) => shades.dark2,
                        (false, ControlState::Pressed) => shades.light3,
                        (false, ControlState::Hover) => shades.light2,
                        _ => shades.light1,
                    };
                    palette.top = palette.border;
                    palette.bottom = if dark {
                        if state == ControlState::Pressed { shades.dark2 } else { shades.light2 }
                    } else if state == ControlState::Pressed {
                        shades.light3
                    } else {
                        shades.dark1
                    };
                } else if primary && !dark {
                    palette.border = fill;
                    palette.top = fill;
                    palette.bottom = fill;
                }
                palette
            }
            Self::Field { selection, focused, state } => {
                let fill = if dark {
                    white(if focused && !selection {
                        10
                    } else if state == ControlState::Hover {
                        21
                    } else {
                        15
                    })
                } else if state == ControlState::Disabled {
                    [249, 249, 249, 76]
                } else if focused && !selection {
                    white(255)
                } else if state == ControlState::Hover {
                    [249, 249, 249, 128]
                } else {
                    white(179)
                };
                let mut palette = uniform(
                    5.0,
                    fill,
                    neutral(if dark { if selection { 14 } else { 20 } } else { 13 }),
                );
                palette.top = neutral(if dark { 20 } else { 13 });
                if !selection {
                    palette.bottom = if focused {
                        shades.primary
                    } else {
                        neutral(if dark { 139 } else { 100 })
                    };
                    if focused {
                        palette.insets[3] = 2.0;
                    }
                }
                palette
            }
            Self::Section => uniform(22.0, neutral(10), neutral(19)),
            Self::ProgressSection => uniform(18.0, neutral(10), neutral(19)),
            Self::IconBadge => uniform(14.0, shades.tint(31), shades.tint(61)),
            Self::Choice { state } => {
                uniform(18.0, neutral(if state == ControlState::Hover { 6 } else { 0 }), neutral(0))
            }
            Self::Summary => uniform(18.0, neutral(6), neutral(0)),
            Self::Card => uniform(
                8.0,
                if dark { white(13) } else { white(179) },
                [0, 0, 0, if dark { 25 } else { 15 }],
            ),
            Self::Footer => Palette {
                radius: 0.0,
                fill: if dark { [32, 32, 32, 255] } else { [243, 243, 243, 255] },
                border: white(0),
                top: [0, 0, 0, 25],
                bottom: white(0),
                insets: [0.0, 1.0, 0.0, 0.0],
            },
            Self::Output => {
                uniform(5.0, if dark { [16, 16, 16, 255] } else { white(255) }, neutral(37))
            }
            Self::Radio { checked } => {
                let mut palette = uniform(
                    10.0,
                    neutral(0),
                    if checked { shades.primary } else { neutral(if dark { 153 } else { 122 }) },
                );
                if checked {
                    palette.insets = [5.0; 4];
                }
                palette
            }
            Self::Check { checked, state } => uniform(
                2.0,
                if checked && state == ControlState::Disabled {
                    neutral(if dark { 41 } else { 56 })
                } else if checked {
                    shades.checkbox(state)
                } else if dark {
                    [59, 59, 59, 255]
                } else {
                    neutral(match state {
                        ControlState::Rest => 6,
                        ControlState::Hover => 13,
                        ControlState::Pressed => 31,
                        ControlState::Disabled => 0,
                    })
                },
                if checked && state == ControlState::Disabled {
                    white(0)
                } else if checked {
                    shades.checkbox(state)
                } else if dark {
                    [133, 133, 133, 255]
                } else {
                    neutral(match state {
                        ControlState::Rest => 122,
                        ControlState::Hover => 143,
                        ControlState::Pressed => 69,
                        ControlState::Disabled => 56,
                    })
                },
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
                fill: if dark { [32, 32, 32, 150] } else { [243, 243, 243, 150] },
                border: white(0),
                top: neutral(18),
                bottom: white(0),
                insets: [0.0, 1.0, 0.0, 0.0],
            },
        }
    }
}
