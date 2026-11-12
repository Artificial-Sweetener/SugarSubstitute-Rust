// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe a committed DirectComposition transaction on an admitted worker.

use std::{
    error::Error,
    fmt,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
};
use sugarsubstitute_contracts::execution::JobContext;
use windows::Win32::Graphics::DirectComposition::IDCompositionDevice;

const PENDING: u8 = 0;
const RUNNING: u8 = 1;
const COMPLETE: u8 = 2;
const FAILED: u8 = 3;

/// The desktop's observation of one already accepted composition commit.
/// Affected native resources remain retained until this fence is complete.
#[derive(Clone)]
pub struct CompositionFence {
    state: Arc<AtomicU8>,
}

/// The exact device reference to transfer to one admitted blocking worker.
pub struct CompositionWait {
    device: Option<IDCompositionDevice>,
    fence: CompositionFence,
    #[cfg(any(test, feature = "boundary-fixtures"))]
    fixture: bool,
}

// SAFETY: DirectComposition objects are explicitly not thread-bound and its API permits
// calls from any thread. This value owns one COM reference, transfers it to one worker,
// and only waits after the UI has committed; the UI retains the original device and
// affected resources until the actual wait returns. No UI-affine object is transferred.
// https://learn.microsoft.com/en-us/windows/win32/directcomp/basic-concepts
unsafe impl Send for CompositionWait {}

/// A compositor wait failed or could not begin within its admitted job lifetime.
#[derive(Debug)]
pub enum CompositionFenceError {
    /// The same native wait was submitted more than once.
    AlreadyStarted,
    /// The job was cancelled before entering the native wait.
    Cancelled,
    /// The job's finite deadline had already expired.
    Deadline,
    /// DirectComposition did not confirm completion.
    Native(windows::core::Error),
}

impl fmt::Display for CompositionFenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyStarted => formatter.write_str("Composition wait already started"),
            Self::Cancelled => formatter.write_str("Composition wait cancelled before entry"),
            Self::Deadline => formatter.write_str("Composition wait deadline expired before entry"),
            Self::Native(cause) => write!(formatter, "Composition wait failed: {cause}"),
        }
    }
}

impl Error for CompositionFenceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Native(cause) => Some(cause),
            _ => None,
        }
    }
}

impl CompositionFence {
    /// Pair one observation with the exact device that accepted the transaction.
    pub(crate) fn new(device: IDCompositionDevice) -> (Self, CompositionWait) {
        let fence = Self { state: Arc::new(AtomicU8::new(PENDING)) };
        (
            fence.clone(),
            CompositionWait {
                device: Some(device),
                fence,
                #[cfg(any(test, feature = "boundary-fixtures"))]
                fixture: false,
            },
        )
    }

    /// Supply a deterministic compositor boundary to application ownership tests.
    #[cfg(any(test, feature = "boundary-fixtures"))]
    #[doc(hidden)]
    pub fn fixture() -> (Self, CompositionWait) {
        let fence = Self { state: Arc::new(AtomicU8::new(PENDING)) };
        (fence.clone(), CompositionWait { device: None, fence, fixture: true })
    }

    /// Observe actual compositor completion; commit acceptance alone never sets this state.
    pub fn completed(&self) -> bool {
        self.state.load(Ordering::Acquire) == COMPLETE
    }

    /// Observe a failed wait without treating it as completion.
    pub fn failed(&self) -> bool {
        self.state.load(Ordering::Acquire) == FAILED
    }

    /// Fence an unstarted or panicked wait after its job has actually reached a terminal outcome.
    pub fn terminal_without_completion(&self) {
        match self.state.fetch_update(Ordering::AcqRel, Ordering::Acquire, |state| {
            (state != COMPLETE).then_some(FAILED)
        }) {
            Ok(_) | Err(_) => {}
        }
    }
}

impl CompositionWait {
    /// Borrow the paired observation while retaining the original native input.
    pub fn fence(&self) -> CompositionFence {
        self.fence.clone()
    }

    /// Wait on the admitted worker for this device's previous commit to be processed.
    /// A later commit may also be covered, but never makes an earlier completion unsafe.
    /// Cancellation cannot interrupt the native call; its actual return remains owned. A job
    /// without a deadline waits for the compositor however long it takes; one whose deadline
    /// passed before entry never starts the call.
    ///
    /// # Errors
    /// Preserves admission lifetime failures and the original DirectComposition error.
    pub fn wait(mut self, context: &JobContext) -> Result<(), CompositionFenceError> {
        self.fence
            .state
            .compare_exchange(PENDING, RUNNING, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| CompositionFenceError::AlreadyStarted)?;
        let device = self.device.take();
        #[cfg(any(test, feature = "boundary-fixtures"))]
        let fixture = std::mem::take(&mut self.fixture);
        #[cfg(not(any(test, feature = "boundary-fixtures")))]
        let fixture = false;
        let result = if context.cancellation_requested() {
            Err(CompositionFenceError::Cancelled)
        } else if context.deadline().is_some_and(|deadline| context.now() >= deadline) {
            Err(CompositionFenceError::Deadline)
        } else if fixture {
            Ok(())
        } else if let Some(device) = &device {
            // SAFETY: DirectComposition device calls are thread-safe. The UI retains the
            // original device and every affected visual and surface through this wait.
            unsafe { device.WaitForCommitCompletion() }.map_err(CompositionFenceError::Native)
        } else {
            Err(CompositionFenceError::AlreadyStarted)
        };
        drop(device);
        self.fence.state.store(if result.is_ok() { COMPLETE } else { FAILED }, Ordering::Release);
        result
    }
}

impl Drop for CompositionWait {
    fn drop(&mut self) {
        let abandoned = self.device.is_some();
        #[cfg(any(test, feature = "boundary-fixtures"))]
        let abandoned = abandoned || self.fixture;
        if abandoned {
            self.fence.terminal_without_completion();
        }
    }
}
