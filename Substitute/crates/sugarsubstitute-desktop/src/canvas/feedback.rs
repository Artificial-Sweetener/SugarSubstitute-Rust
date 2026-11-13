// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish tool feedback synchronously without coupling native input to frame production.
use super::*;
#[cfg(windows)]
use std::{path::PathBuf, time::Duration};
use sugarsubstitute_canvas::CursorIntent;
#[cfg(windows)]
use sugarsubstitute_image_ops::Extent;

/// Narrow application boundary required to derive native feedback from one input mutation.
pub(super) trait FeedbackOwner {
    /// Apply one normalized observation without native or presentation ownership.
    /// # Errors
    /// Preserves the application's original input failure.
    fn apply_input(&mut self, input: Input) -> Result<(), Box<dyn Error>>;
    /// Read the cursor and capture intent resulting from all completed mutations.
    fn current_feedback(&self) -> Feedback;
}

impl<T: Lifetime> FeedbackOwner for T {
    fn apply_input(&mut self, input: Input) -> Result<(), Box<dyn Error>> {
        self.input(input)
    }

    fn current_feedback(&self) -> Feedback {
        self.feedback()
    }
}

/// Native operations needed to publish feedback without exposing window or cursor resources.
pub(super) trait FeedbackTarget {
    /// Select one already-resolved toolkit-independent cursor intent.
    /// # Errors
    /// Preserves native cursor-resource and selection failures.
    fn publish_cursor(&self, cursor: CursorIntent) -> Result<(), Box<dyn Error>>;
    /// Reconcile pointer capture, returning false when another native owner prevents acquisition.
    /// # Errors
    /// Preserves native capture query and release failures.
    fn reconcile_capture(&self, capture: bool) -> Result<bool, Box<dyn Error>>;
}

/// Apply one normalized input and publish its resulting cursor and capture before returning.
/// # Errors
/// Preserves application input, cursor-resource and native capture failures.
pub(super) fn dispatch_input(
    owner: &mut impl FeedbackOwner,
    target: Option<&impl FeedbackTarget>,
    input: Input,
) -> Result<(), Box<dyn Error>> {
    owner.apply_input(input)?;
    synchronize(owner, target)
}

/// Resize application geometry and publish any resulting cursor or capture change.
/// # Errors
/// Preserves geometry, cursor-resource and native capture failures.
#[cfg(windows)]
pub(super) fn resize(
    owner: &mut impl Lifetime,
    target: Option<&impl FeedbackTarget>,
    extent: Extent,
) -> Result<(), Box<dyn Error>> {
    owner.resize(extent)?;
    synchronize(owner, target)
}

/// Observe background state and publish feedback before callers inspect or present a frame.
/// # Errors
/// Preserves polling, cursor-resource and native capture failures.
#[cfg(windows)]
pub(super) fn poll(
    owner: &mut impl Lifetime,
    target: Option<&impl FeedbackTarget>,
    elapsed: Duration,
) -> Result<(), Box<dyn Error>> {
    owner.poll(elapsed)?;
    synchronize(owner, target)
}

/// Cancel application state and release native feedback before teardown continues.
/// # Errors
/// Preserves shutdown, cursor-resource and native capture failures.
#[cfg(windows)]
pub(super) fn close(
    owner: &mut impl Lifetime,
    target: Option<&impl FeedbackTarget>,
) -> Result<(), Box<dyn Error>> {
    owner.close()?;
    synchronize(owner, target)
}

/// Apply a picker result and publish document-dependent tool feedback before polling work.
/// # Errors
/// Preserves selection, cursor-resource and native capture failures.
#[cfg(windows)]
pub(super) fn selected(
    owner: &mut impl Lifetime,
    target: Option<&impl FeedbackTarget>,
    selection: Result<Option<PathBuf>, Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    owner.selected(selection)?;
    synchronize(owner, target)
}

/// Publish current feedback immediately. Rejected capture causes one nonrecursive cancellation
/// and a second publication so no native gesture or closed cursor remains logically active.
/// # Errors
/// Preserves cursor-resource, native capture and cancellation-input failures.
pub(super) fn synchronize(
    owner: &mut impl FeedbackOwner,
    target: Option<&impl FeedbackTarget>,
) -> Result<(), Box<dyn Error>> {
    let Some(target) = target else {
        return Ok(());
    };
    let feedback = owner.current_feedback();
    target.publish_cursor(feedback.cursor)?;
    if target.reconcile_capture(feedback.capture)? {
        return Ok(());
    }

    owner.apply_input(Input::Unfocused)?;
    let released = owner.current_feedback();
    target.publish_cursor(released.cursor)?;
    if target.reconcile_capture(released.capture)? {
        Ok(())
    } else {
        Err("Native pointer capture remained unavailable after cancellation".into())
    }
}

#[cfg(test)]
mod tests;
