// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Replay a closed and reopened output pane through staged reattachment and a later selection.
//! The host loop advances the injected clock only for deadlines the canvas actually reports.

use super::*;
use host::{Host, geometry, select};

/// When the page withdraws the first hidden-candidate selection relative to its worker progress.
#[derive(Clone, Copy, Debug)]
enum SelectionCancel {
    /// Before any completion was drained for the selection.
    Queued,
    /// After the selected source loaded but before its pixels were prepared.
    Loaded,
    /// After the host already replied; the cancellation only consumes its receipt.
    Replied,
}

/// Controllable orderings of the reopened pane's protocol record.
#[derive(Clone, Copy, Debug)]
struct ReopenOrder {
    /// Release the committed attachment before staging, or stage while it is still retained.
    release_first: bool,
    /// CSS viewport of the first hidden candidate geometries; a zero extent is undrawable.
    hidden: (u32, u32),
    cancel: SelectionCancel,
    /// Drain all refinement and retirement work after the staged commit before the next request.
    quiesce: bool,
    /// Injected page time in milliseconds before each request through the staged commit.
    page_delay: u64,
    /// Injected time before the viewport read and the user's later click.
    click_delay: u64,
}

/// Replay the recorded reopened-pane sequence and require the final selection to present B.
fn replay(order: ReopenOrder) -> Result<(), Box<dyn Error>> {
    let mut replay = Host::new(order.page_delay)?;
    let result = reopen_and_select(&mut replay, order);
    let finished = replay.finish();
    result.and(finished)
}

fn reopen_and_select(replay: &mut Host, order: ReopenOrder) -> Result<(), Box<dyn Error>> {
    replay.send(1, 4, "surface.commit", json!({}))?;
    replay.expect(4, "applied")?;
    replay.quiesce()?;
    if order.release_first {
        replay.send(1, 5, "surface.release", json!({}))?;
        replay.expect(5, "applied")?;
        replay.send(4, 6, "surface.prepare", json!({}))?;
        replay.expect(6, "ready")?;
    } else {
        replay.send(4, 5, "surface.prepare", json!({}))?;
        replay.expect(5, "ready")?;
        replay.send(1, 6, "surface.release", json!({}))?;
        replay.expect(6, "stale")?;
    }
    let (width, height) = order.hidden;
    for id in [9, 10] {
        replay.send(4, id, "surface.geometry", geometry(width, height, false))?;
        replay.expect(id, "applied")?;
        assert_eq!(replay.cancel(id)?, None);
    }
    replay.send(4, 11, "surface.geometry", geometry(width, height, false))?;
    replay.expect(11, "applied")?;
    replay.send(4, 12, "surface.select", select("wide.png"))?;
    let pending = match order.cancel {
        SelectionCancel::Queued => true,
        SelectionCancel::Loaded => replay.until_loaded(12)?,
        SelectionCancel::Replied => {
            replay.expect(12, "applied")?;
            false
        }
    };
    let cancelled = replay.cancel(12)?;
    assert_eq!(cancelled.as_deref(), pending.then_some("cancelled"));
    let (width, height) = if width == 0 || height == 0 { (80, 40) } else { (width, height) };
    replay.send(4, 13, "surface.geometry", geometry(width, height, false))?;
    replay.expect(13, "applied")?;
    replay.send(4, 14, "surface.select", select("wide.png"))?;
    replay.expect(14, "applied")?;
    replay.send(4, 15, "surface.geometry", geometry(80, 40, true))?;
    replay.expect(15, "applied")?;
    replay.send(4, 16, "surface.commit", json!({}))?;
    replay.expect(16, "applied")?;
    if order.quiesce {
        replay.quiesce()?;
    }
    replay.page_delay = order.click_delay;
    replay.send(4, 17, "surface.viewport.request", json!({"command":{"type":"read"}}))?;
    replay.expect(17, "applied")?;
    replay.send(4, 18, "surface.select", select("tall.png"))?;
    replay.expect(18, "applied")?;
    let accepted = replay.fixture.accepted()?;
    assert_eq!(accepted.extent, sugarsubstitute_image_ops::Extent::new(160, 320)?);
    let shown = match replay.fixture.canvas.presentation() {
        MainCanvasPresentation::Frame(frame) if !frame.content.pending() => frame.content.target(),
        MainCanvasPresentation::Retain(retained) => retained.target,
        _ => return Err("The final selection is not the acknowledged native presentation".into()),
    };
    assert_eq!(shown, accepted.frame);
    replay.quiesce()
}

/// Cover drawable and undrawable hidden candidates, first-selection cancellation points and
/// post-commit settlement. Requests arrive back to back, as recorded, or one display period apart.
fn orderings(release_first: bool) -> Result<(), Box<dyn Error>> {
    let mut failures = Vec::new();
    for hidden in [(0, 0), (80, 40)] {
        for cancel in [SelectionCancel::Queued, SelectionCancel::Loaded, SelectionCancel::Replied] {
            // An undrawable candidate never prepares pixels, so only the page can retire it.
            if hidden == (0, 0) && matches!(cancel, SelectionCancel::Replied) {
                continue;
            }
            for quiesce in [false, true] {
                for (page_delay, click_delay) in [(0, 0), (100, 100)] {
                    let order = ReopenOrder {
                        release_first,
                        hidden,
                        cancel,
                        quiesce,
                        page_delay,
                        click_delay,
                    };
                    if let Err(cause) = replay(order) {
                        failures.push(format!("{order:?}: {cause}"));
                    }
                }
            }
        }
    }
    if failures.is_empty() { Ok(()) } else { Err(failures.join("\n").into()) }
}

#[test]
fn reopened_pane_presents_a_later_selection_after_releasing_before_staging()
-> Result<(), Box<dyn Error>> {
    orderings(true)
}

#[test]
fn reopened_pane_presents_a_later_selection_while_the_old_binding_is_retained()
-> Result<(), Box<dyn Error>> {
    orderings(false)
}
