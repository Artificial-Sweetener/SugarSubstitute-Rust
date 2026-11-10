// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hold a selection's worker-side acquisition until the test releases it.

use super::*;
use crate::native_canvas::session::{SelectedOriginals, acquisition::SourceAcquirer};
use std::{
    io,
    path::PathBuf,
    sync::{Condvar, Mutex},
};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_image_io::ImageSource;
use sugarsubstitute_supervision::cache::RuntimeCache;

#[derive(Default)]
struct State {
    entered: bool,
    returned: bool,
    released: bool,
    cancelled: bool,
    settled: bool,
    retirements: usize,
}

/// One fixture original whose acquisition blocks on an explicit release.
///
/// A cooperative gate returns `Interrupted` once cancelled, as a real transfer does. A
/// non-cooperative gate ignores cancellation and still returns its original when released,
/// so a superseded acquisition can complete successfully after a newer selection.
pub(super) struct Gate {
    path: PathBuf,
    /// The host loop's wake, signalled when a worker enters or leaves the acquisition.
    wake: Arc<Wake>,
    cooperative: bool,
    state: Mutex<State>,
    changed: Condvar,
}

impl Gate {
    /// Acquire `path` only after `release`; `cooperative` gates also stop on cancellation.
    pub(super) fn new(path: PathBuf, cooperative: bool, wake: Arc<Wake>) -> Arc<Self> {
        Arc::new(Self {
            path,
            wake,
            cooperative,
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
        })
    }

    /// Let the blocked acquisition return its original.
    pub(super) fn release(&self) {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).released = true;
        self.changed.notify_all();
    }

    /// Let a blocked acquisition that ignores cancellation return; a cooperative gate stops on
    /// cancellation instead, so closing the canvas settles it.
    pub(super) fn release_if_uncooperative(&self) {
        if !self.cooperative {
            self.release();
        }
    }

    /// Report whether a worker actually entered the acquisition.
    pub(super) fn entered(&self) -> bool {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).entered
    }

    /// Report whether the acquisition returned to its loading job.
    pub(super) fn returned(&self) -> bool {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).returned
    }

    /// Report whether the canvas cancelled this acquisition.
    pub(super) fn cancelled(&self) -> bool {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).cancelled
    }

    /// Report how many separate worker retirements this acquisition received.
    pub(super) fn retirements(&self) -> usize {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).retirements
    }

    fn stopped(&self, state: &State) -> bool {
        self.cooperative && state.cancelled
    }
}

impl SourceAcquirer for Gate {
    fn reservation_bytes(&self) -> u64 {
        1024
    }

    fn acquire(
        &self,
        _: RuntimeCache,
        _: &MemoryLease,
        _: &JobContext,
    ) -> io::Result<SelectedOriginals> {
        let mut state = self.state.lock().map_err(|_| io::Error::other("Gate state poisoned"))?;
        state.entered = true;
        self.wake.wake().map_err(io::Error::other)?;
        let mut state = self
            .changed
            .wait_while(state, |state| !state.released && !self.stopped(state))
            .map_err(|_| io::Error::other("Gate state poisoned"))?;
        state.returned = true;
        self.wake.wake().map_err(io::Error::other)?;
        if self.stopped(&state) {
            return Err(io::ErrorKind::Interrupted.into());
        }
        state.settled = true;
        Ok(SelectedOriginals::single(ImageSource::file(self.path.clone())))
    }

    fn cancel(&self) {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).cancelled = true;
        self.changed.notify_all();
    }

    fn retire(&self, _: &JobContext) -> io::Result<()> {
        let mut state = self.state.lock().map_err(|_| io::Error::other("Gate state poisoned"))?;
        state.retirements += 1;
        state.settled = true;
        Ok(())
    }

    fn settled(&self) -> bool {
        self.state.lock().map(|state| state.settled).unwrap_or(false)
    }
}
