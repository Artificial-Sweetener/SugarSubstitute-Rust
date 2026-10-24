// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Choose only visible owned peers in caption, output, then lower-action order.
use crate::AccessibleControl;
/// Presentation-only target; the affine adapter resolves it against its retained native inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Target {
    Control(u16),
    Console,
}
fn ordered(
    controls: impl Iterator<Item = AccessibleControl> + Clone,
    console_y: Option<f64>,
) -> impl Iterator<Item = Target> {
    let before = controls
        .clone()
        .filter(move |control| {
            control.availability().actionable() && console_y.is_none_or(|y| control.bounds.y < y)
        })
        .map(|control| Target::Control(control.id));
    let after = controls
        .filter(move |control| {
            control.availability().actionable() && console_y.is_some_and(|y| control.bounds.y >= y)
        })
        .map(|control| Target::Control(control.id));
    before.chain(console_y.map(|_| Target::Console)).chain(after)
}
/// Wrap in either direction without activating controls or producing a foreign target.
pub(crate) fn next(
    controls: impl Iterator<Item = AccessibleControl> + Clone,
    console_y: Option<f64>,
    current: Option<Target>,
    backward: bool,
) -> Option<Target> {
    let count = ordered(controls.clone(), console_y).count();
    if count == 0 {
        return None;
    }
    let at = current.and_then(|current| {
        ordered(controls.clone(), console_y).position(|target| target == current)
    });
    let next = match (at, backward) {
        (Some(0), true) | (None, true) => count - 1,
        (Some(at), true) => at - 1,
        (Some(at), false) => (at + 1) % count,
        (None, false) => 0,
    };
    ordered(controls, console_y).nth(next)
}
#[cfg(test)]
mod tests;
