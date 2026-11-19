// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Lay out the main window's caption peers: the window buttons at the right of the page's own
//! title bar row. Peer identities never change; only bounds and the maximize label do. The
//! caption carries nothing else, so the page fills the client and never moves.
use crate::{AccessibleControl, ControlBounds, MainCaptionLabels};

/// Every caption peer, in identity order.
pub(crate) const PEERS: usize = 3;
/// Window buttons are 46 logical pixels wide, like the Windows caption buttons.
const BUTTON_WIDTH: u16 = 46;
const BUTTON: f64 = BUTTON_WIDTH as f64;
/// The window buttons' width, which the page leaves free at the right of its title bar row.
pub(crate) const CAPTION_WIDTH: u16 = 3 * BUTTON_WIDTH;
/// The caption strip's height in logical pixels; the page's own title bar row matches it.
pub(crate) const TITLEBAR_HEIGHT: u16 = 34;
/// Caption row top and height in logical pixels.
const TOP: f64 = 1.0;
const HEIGHT: f64 = 32.0;

/// Caption peers for a client `width` logical pixels wide.
pub(crate) fn caption_controls(
    width: f64,
    maximized: bool,
    labels: MainCaptionLabels,
) -> [AccessibleControl; PEERS] {
    let buttons = width - 3.0 * BUTTON;
    let peer = |id: u16, label: &'static str| AccessibleControl {
        enabled: true,
        id,
        label,
        bounds: ControlBounds {
            x: buttons + f64::from(id - 1) * BUTTON,
            y: TOP,
            width: BUTTON,
            height: HEIGHT,
        },
        close: id == 3,
        visible: true,
    };
    [
        peer(1, labels.minimize),
        peer(2, if maximized { labels.restore } else { labels.maximize }),
        peer(3, labels.close),
    ]
}

#[cfg(test)]
#[path = "main_caption_tests.rs"]
mod tests;
