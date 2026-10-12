// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain renderer or immutable request ownership across rejected or cancelled factories.

use std::{
    error::Error,
    fmt,
    sync::{Arc, Mutex},
};

/// Transfer owned preparation input only when an admitted worker invokes its factory.
///
/// A rejected or uninvoked factory drops only its shared transfer handle. The
/// application retains the input without calling its destructor on the UI
/// thread. The application never takes while an accepted job is running; its
/// terminal outcome first establishes whether the factory consumed ownership.
pub(crate) struct PreparationTransfer<T>(Arc<Mutex<Option<T>>>);

impl<T> Clone for PreparationTransfer<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T> PreparationTransfer<T> {
    /// Keep idle owned state available until an accepted preparation begins.
    pub(crate) fn new(value: T) -> Self {
        Self(Arc::new(Mutex::new(Some(value))))
    }

    /// Move state exactly once without invoking renderer code while locked.
    ///
    /// On a worker, call from its admitted factory. On the application thread,
    /// recover only after rejection or actual queued cancellation/coalescing.
    /// A consumed transfer identifies lost state after a supervised job failure.
    pub(crate) fn take(&self) -> Result<T, TransferError> {
        self.0.lock().map_err(|_| TransferError::Poisoned)?.take().ok_or(TransferError::Consumed)
    }
}

/// Observe unavailable transfer state before any renderer use.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TransferError {
    /// A worker already took the renderer; only its terminal result can return it.
    Consumed,
    /// The transfer's ownership invariant is uncertain after lock poisoning.
    Poisoned,
}

impl fmt::Display for TransferError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Consumed => "The preparation factory consumed input ownership",
            Self::Poisoned => "Preparation transfer ownership is uncertain",
        })
    }
}
impl Error for TransferError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Renderer(Arc<AtomicUsize>);
    impl Drop for Renderer {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn uninvoked_factory_drop_keeps_owned_renderer_available() -> Result<(), TransferError> {
        let drops = Arc::new(AtomicUsize::new(0));
        let transfer = PreparationTransfer::new(Renderer(drops.clone()));
        let captured = transfer.clone();
        let factory = move || captured.take();
        drop(factory);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        let renderer = transfer.take()?;
        drop(transfer);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        drop(renderer);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        Ok(())
    }

    #[test]
    fn factory_takes_the_same_state_once_and_cannot_duplicate_it() -> Result<(), TransferError> {
        let drops = Arc::new(AtomicUsize::new(0));
        let transfer = PreparationTransfer::new(Renderer(drops.clone()));
        let captured = transfer.clone();
        let factory = move || captured.take();
        let renderer = factory()?;
        assert!(matches!(transfer.take(), Err(TransferError::Consumed)));
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        drop(renderer);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        Ok(())
    }
}
