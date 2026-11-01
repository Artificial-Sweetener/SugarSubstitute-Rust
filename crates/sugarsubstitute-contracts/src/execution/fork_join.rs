// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Spread one admitted job's borrowed work without exposing threads, queues or a scheduler.

/// One borrowed, independent piece of a fork-join call.
pub type ForkJoinItem<'a> = Box<dyn FnOnce() + Send + 'a>;

/// Run borrowed work items to completion on execution-owned, bounded helper capacity.
///
/// Only the execution owner implements this port and only CPU job contexts carry it. A call
/// returns after every item has finished, so no item outlives the job step that started it.
pub trait ForkJoin: Send + Sync {
    /// Maximum items that run at once, including the calling worker.
    fn width(&self) -> usize;

    /// Run every item, using helpers that are free and the caller for the rest.
    ///
    /// Returns only after all items finished. A panic from any item resumes on the caller
    /// after the remaining items complete.
    fn join<'a>(&self, items: Vec<ForkJoinItem<'a>>);
}
