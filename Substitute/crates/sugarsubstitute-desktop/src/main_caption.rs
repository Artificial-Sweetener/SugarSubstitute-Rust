// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Lay out the main window's caption peers: the window buttons at the right, the update
//! button and its restart offer beside them, and the backend retry or one text line at the
//! left. Peer identities never change; only bounds and availability do.
use crate::{
    AccessibleControl, ControlBounds, MainCaptionLabels,
    main_backend::RetryPresentation,
    main_updates::{DISMISS, LINE, LINK, OFFER, UPDATE, UpdateCaptionView},
};

/// Every caption peer, in identity order.
pub(crate) const PEERS: usize = 9;
/// Window and update buttons are 46 logical pixels wide, like the Windows caption buttons.
const BUTTON: f64 = 46.0;
/// Caption row top and height in logical pixels.
const TOP: f64 = 1.0;
const HEIGHT: f64 = 32.0;
/// Space around a text peer's words, and between neighbouring peers.
const TEXT_PADDING: f64 = 16.0;
const GAP: f64 = 4.0;
/// The left edge of the retry and text line.
const LEFT: f64 = 8.0;
/// The narrowest text peer worth showing.
const MINIMUM_TEXT: f64 = 48.0;

/// Caption peers for a client `width` logical pixels wide. `measure` returns the logical
/// width of a text peer's words in the caption font.
/// # Errors
/// Preserves the first measurement failure.
pub(crate) fn caption_controls<E>(
    width: f64,
    maximized: bool,
    labels: MainCaptionLabels,
    retry: RetryPresentation,
    updates: &UpdateCaptionView,
    measure: impl Fn(&str) -> Result<f64, E>,
) -> Result<[AccessibleControl; PEERS], E> {
    let peer =
        |id: u16, label: &'static str, x: f64, width: f64, visible: bool| AccessibleControl {
            enabled: true,
            id,
            label,
            bounds: ControlBounds { x, y: TOP, width: width.max(1.0), height: HEIGHT },
            close: id == 3,
            visible,
        };
    let buttons = width - 3.0 * BUTTON;
    let update_x = buttons - BUTTON;
    let text_width = |text: &str| Ok(measure(text)?.ceil() + TEXT_PADDING);
    let offer_width = text_width(&updates.offer.text)?;
    let offer_x = update_x - offer_width;
    let right_edge = if updates.offer.visible {
        offer_x
    } else if updates.update.visible {
        update_x
    } else {
        buttons
    } - GAP;
    // The retry button owns the left of the caption while the backend is unavailable.
    let line_room = !retry.visible;
    let link_width = if updates.link.visible { text_width(&updates.link.text)? } else { 0.0 };
    let dismiss_width =
        if updates.dismiss.visible { text_width(&updates.dismiss.text)? } else { 0.0 };
    let line_width = text_width(&updates.line.text)?
        .min(right_edge - LEFT - link_width - dismiss_width)
        .max(0.0);
    let line_fits = line_room && line_width >= MINIMUM_TEXT;
    let link_x = LEFT + line_width;
    let dismiss_x = link_x + link_width;
    Ok([
        peer(1, labels.minimize, buttons, BUTTON, true),
        peer(
            2,
            if maximized { labels.restore } else { labels.maximize },
            buttons + BUTTON,
            BUTTON,
            true,
        ),
        peer(3, labels.close, buttons + 2.0 * BUTTON, BUTTON, true),
        peer(4, retry.label, LEFT, (width - 154.0).clamp(144.0, 480.0), retry.visible),
        peer(UPDATE, updates.update.label, update_x, BUTTON, updates.update.visible),
        peer(OFFER, updates.offer.label, offer_x, offer_width, updates.offer.visible),
        peer(LINE, updates.line.label, LEFT, line_width, line_fits && updates.line.visible),
        peer(LINK, updates.link.label, link_x, link_width, line_fits && updates.link.visible),
        peer(
            DISMISS,
            updates.dismiss.label,
            dismiss_x,
            dismiss_width,
            line_fits && updates.dismiss.visible,
        ),
    ])
}

#[cfg(test)]
#[path = "main_caption_tests.rs"]
mod tests;
