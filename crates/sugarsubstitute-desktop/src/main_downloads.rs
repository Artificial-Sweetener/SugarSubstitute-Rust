// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound browser-owned downloads to the displayed frontend and retain native cancellation custody.

use crate::DesktopError;
use std::cell::{Cell, RefCell};
use sugarsubstitute_contracts::configuration::LoopbackOrigin;

/// Include paused/resumable downloads in the same bounded native operation inventory.
pub(crate) const MAX_DOWNLOADS: usize = 8;

/// Native state needed for custody; interruption alone does not prove terminal cancellation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DownloadState {
    Retained,
    Terminal,
}

/// A creating-thread browser operation, with native getters/effects outside storage borrows.
/// Cloning copies an authored ownership handle; it must not enter native code.
pub(crate) trait DownloadOperation: Clone {
    /// Return terminal only for completion or an explicit user-cancelled interruption.
    fn state(&self) -> Result<DownloadState, DesktopError>;
    /// Request cancellation once; a later state observation must establish terminal state.
    fn cancel(&self) -> Result<(), DesktopError>;
}

struct Slot<O> {
    identity: u64,
    operation: O,
    cancel_requested: bool,
}

/// Retain at most eight browser operations through actual observed completion/cancellation.
///
/// Native calls can pump callbacks. Copy the operation and its monotonic identity before
/// every call, then match that identity before removing storage. Closing is irreversible.
pub(crate) struct Downloads<O> {
    slots: RefCell<[Option<Slot<O>>; MAX_DOWNLOADS]>,
    next: Cell<u64>,
    closing: Cell<bool>,
    next_retirement: Cell<usize>,
    uncertain: Cell<bool>,
}

impl<O: DownloadOperation> Downloads<O> {
    /// Prepare fixed operation slots without opening files or creating workers.
    pub(crate) fn new() -> Self {
        Self {
            slots: RefCell::new(std::array::from_fn(|_| None)),
            next: Cell::new(1),
            closing: Cell::new(false),
            next_retirement: Cell::new(0),
            uncertain: Cell::new(false),
        }
    }

    /// Adopt before allowing the browser to save; overload leaves the event cancelled.
    pub(crate) fn retain(&self, operation: O) -> Result<bool, DesktopError> {
        if self.closing.get() {
            return Ok(false);
        }
        let mut input = Some(operation);
        Ok(self.adopt(&mut input)?.is_some())
    }

    /// A failed event cancellation adopts a returned reference into an available funded slot.
    /// Missing or saturated partial outputs keep the native Core behind a sticky uncertainty fence.
    /// This does not admit a ninth download or turn a successful Cancel request into a terminal.
    pub(crate) fn cancel_unadmitted(
        &self,
        cancel_event: impl FnOnce() -> Result<(), DesktopError>,
        acquire: impl FnOnce() -> (Option<O>, Result<(), DesktopError>),
    ) -> Result<(), DesktopError> {
        let Err(first) = cancel_event() else { return Ok(()) };
        self.closing.set(true);
        let (mut operation, acquired) = acquire();
        let mut failure =
            StartingFailure { first, getter: acquired.err(), admission: None, cancel: None };
        if failure.getter.is_some() || operation.is_none() {
            self.uncertain.set(true);
        }
        if operation.is_none() {
            if failure.getter.is_none() {
                failure.getter =
                    Some(DesktopError::message("get_browser_download", "native operation missing"));
            }
        } else {
            match self.adopt(&mut operation) {
                Ok(Some((index, identity))) => {
                    if let Some(original) = self.cancel_once(index, identity) {
                        failure.cancel = original.cancel().err();
                    }
                }
                rejected => {
                    self.uncertain.set(true);
                    failure.admission = rejected.err();
                    if let Some(original) = operation {
                        failure.cancel = original.cancel().err();
                    }
                }
            }
        }
        Err(DesktopError::new("cancel_unadmitted_download", failure))
    }

    /// Keep the containing Core owned when an actual callback/output cannot be identified.
    /// This irreversible fence never prevents cancellation of the known exact slots.
    pub(crate) fn retain_unknown_native_effect(&self) {
        self.closing.set(true);
        self.uncertain.set(true);
    }

    /// Move an actual callback output only into one of the same eight funded slots.
    /// Rejection preserves the original reference in the caller, with no native effects.
    fn adopt(&self, input: &mut Option<O>) -> Result<Option<(usize, u64)>, DesktopError> {
        let mut slots = self.slots.borrow_mut();
        let Some((index, slot)) = slots.iter_mut().enumerate().find(|(_, slot)| slot.is_none())
        else {
            return Ok(None);
        };
        let identity = self.next.get();
        let next = identity.checked_add(1).ok_or_else(|| {
            DesktopError::message("admit_browser_download", "download identity exhausted")
        })?;
        let Some(operation) = input.take() else { return Ok(None) };
        self.next.set(next);
        *slot = Some(Slot { identity, operation, cancel_requested: false });
        Ok(Some((index, identity)))
    }

    /// Latch the actual attempt before its native call can reenter this owner.
    fn cancel_once(&self, index: usize, identity: u64) -> Option<O> {
        let mut slots = self.slots.borrow_mut();
        let slot = slots[index]
            .as_mut()
            .filter(|slot| slot.identity == identity && !slot.cancel_requested)?;
        slot.cancel_requested = true;
        Some(slot.operation.clone())
    }

    /// Free only terminal slots; paused, resumable, and uncertain operations keep custody.
    pub(crate) fn reap_terminal(&self) -> Result<(), DesktopError> {
        for index in 0..MAX_DOWNLOADS {
            let observed = self.slots.borrow()[index]
                .as_ref()
                .map(|slot| (slot.identity, slot.operation.clone()));
            if let Some((identity, operation)) = observed
                && operation.state()? == DownloadState::Terminal
            {
                self.release(index, identity);
            }
        }
        Ok(())
    }

    /// Close admission and advance one actual operation without waiting or retrying Cancel.
    /// A true result requires every retained operation terminal and no unknown native effect.
    pub(crate) fn retire_step(&self) -> Result<bool, DesktopError> {
        self.closing.set(true);
        let first = self.next_retirement.get();
        let observed = {
            let slots = self.slots.borrow();
            (0..MAX_DOWNLOADS).map(|offset| (first + offset) % MAX_DOWNLOADS).find_map(|index| {
                slots[index].as_ref().map(|slot| {
                    (index, slot.identity, slot.operation.clone(), slot.cancel_requested)
                })
            })
        };
        let Some((index, identity, operation, cancelled)) = observed else {
            return Ok(!self.uncertain.get());
        };
        // Advance before a fallible/reentrant getter; one bad operation cannot starve another.
        self.next_retirement.set((index + 1) % MAX_DOWNLOADS);
        if operation.state()? == DownloadState::Terminal {
            self.release(index, identity);
        } else if !cancelled && let Some(original) = self.cancel_once(index, identity) {
            original.cancel()?;
        }
        Ok(false)
    }

    /// Remove only the operation observed before a possibly reentrant native getter.
    fn release(&self, index: usize, identity: u64) {
        let released = {
            let mut slots = self.slots.borrow_mut();
            if slots[index].as_ref().is_some_and(|slot| slot.identity == identity) {
                slots[index].take()
            } else {
                None
            }
        };
        drop(released);
    }
}

/// Preserve the original property failure and each independent partial-output/cancellation cause.
struct StartingFailure {
    first: DesktopError,
    getter: Option<DesktopError>,
    admission: Option<DesktopError>,
    cancel: Option<DesktopError>,
}
impl std::fmt::Debug for StartingFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StartingFailure")
            .field("getter", &self.getter.is_some())
            .field("admission", &self.admission.is_some())
            .field("cancel", &self.cancel.is_some())
            .finish_non_exhaustive()
    }
}
impl std::fmt::Display for StartingFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Original event cancellation failed with partial download custody retained")
    }
}
impl std::error::Error for StartingFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.first)
    }
}

/// Admit served files and frontend-created blobs only after the actual main reveal.
/// Browser download paths, collision handling, progress and user cancellation stay native.
pub(crate) fn admits_download(
    origin: LoopbackOrigin,
    revealed: bool,
    accepting: bool,
    source: &str,
    uri: &str,
) -> bool {
    revealed
        && accepting
        && crate::main_navigation::admits_url(origin, source)
        && uri.len() <= crate::main_navigation::MAX_URL_BYTES
        && crate::main_navigation::admits_url(origin, uri.strip_prefix("blob:").unwrap_or(uri))
}

#[cfg(test)]
#[path = "main_downloads_tests.rs"]
mod tests;
