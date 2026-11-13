// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Share one UI-only collector without retaining a borrow across native reentry.

use super::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Keep counter, bounded storage and first failure alive across subclass reference transfer.
#[derive(Clone)]
pub(crate) struct TimingObserver {
    audit: Rc<RefCell<Option<TimingAudit>>>,
    failure: Rc<Cell<Option<DesktopError>>>,
    failure_wake_pending: Rc<Cell<bool>>,
}
impl TimingObserver {
    /// Reserve native buffers only for an explicitly requested Windows qualification.
    pub(crate) fn native(
        enabled: bool,
        order: NativeStartupOrder,
    ) -> Result<Option<Self>, DesktopError> {
        if !enabled {
            return Ok(None);
        }
        #[cfg(target_os = "windows")]
        {
            Ok(Some(Self::new(TimingAudit::new(
                Box::new(crate::windows_timing::WindowsClock::new()?),
                order,
            )?)))
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = order;
            Err(error("raw QPC timing is supported only on Windows"))
        }
    }
    /// Construct a UI-only shared sink from already allocated collector storage.
    #[cfg(any(target_os = "windows", test))]
    pub(crate) fn new(audit: TimingAudit) -> Self {
        Self {
            audit: Rc::new(RefCell::new(Some(audit))),
            failure: Rc::new(Cell::new(None)),
            failure_wake_pending: Rc::new(Cell::new(false)),
        }
    }

    /// Release the short storage borrow before callers invoke any native boundary.
    fn with<R>(
        &self,
        observe: impl FnOnce(&mut TimingAudit) -> Result<R, DesktopError>,
    ) -> Result<R, DesktopError> {
        let mut slot = self
            .audit
            .try_borrow_mut()
            .map_err(|cause| DesktopError::new("borrow_native_timing", cause))?;
        let audit = slot.as_mut().ok_or_else(|| error("timing ownership already transferred"))?;
        observe(audit)
    }
    /// Validate the current source/thread tick without lending collector storage.
    pub(crate) fn mark(&self) -> Result<Stamp, DesktopError> {
        self.with(TimingAudit::mark)
    }
    /// Commit one ordered startup stage with no collector borrow across its native boundary.
    pub(crate) fn startup(&self, stage: NativeStartupStage) -> Result<(), DesktopError> {
        self.with(|audit| audit.startup(stage))
    }
    /// Observe the first actual content request only; later calls perform no counter read.
    pub(crate) fn first_preparation(&self) -> Result<(), DesktopError> {
        self.with(TimingAudit::first_preparation)
    }
    /// Append a checked callback completion to preallocated storage.
    pub(crate) fn callback(
        &self,
        begin: Stamp,
        kind: NativeCallbackKind,
        delegated: Option<NativeTickInterval>,
    ) -> Result<(), DesktopError> {
        self.with(|audit| audit.callback(begin, kind, delegated))
    }
    /// Append a checked actual successful frame submission.
    pub(crate) fn submission(
        &self,
        begin: Stamp,
        identity: FrameIdentity,
    ) -> Result<(), DesktopError> {
        self.with(|audit| audit.submission(begin, identity))
    }
    /// Retain a native callback error for the next Tao callback or final teardown check.
    #[cfg(any(target_os = "windows", test))]
    pub(crate) fn fail(&self, error: DesktopError) {
        let mut failure = self.failure.take();
        DesktopError::retain_first(&mut failure, error);
        if let Some(reentered) = self.failure.take() {
            DesktopError::retain_first(&mut failure, reentered);
        }
        self.failure.set(failure);
    }
    /// Inspect retained callback failure without dropping or lending its cause.
    #[cfg(any(target_os = "windows", test))]
    pub(crate) fn has_failure(&self) -> bool {
        let failure = self.failure.take();
        let present = failure.is_some();
        self.failure.set(failure);
        present
    }
    /// Admit at most one native wake for a pending timing-failure batch.
    #[cfg(any(target_os = "windows", test))]
    pub(crate) fn request_failure_wake(&self) -> bool {
        self.has_failure() && !self.failure_wake_pending.replace(true)
    }
    /// Transfer an FFI callback failure into the authored Closing state.
    pub(crate) fn take_failure(&self) -> Option<DesktopError> {
        self.failure_wake_pending.set(false);
        self.failure.take()
    }
    /// Consume the collector only after all installed subclass calls and native teardown return.
    pub(crate) fn finish(self) -> Result<NativeTimingReceipt, DesktopError> {
        if let Some(cause) = self.take_failure() {
            return Err(cause);
        }
        self.audit
            .try_borrow_mut()
            .map_err(|cause| DesktopError::new("borrow_final_timing", cause))?
            .take()
            .ok_or_else(|| error("timing ownership already transferred"))?
            .finish()
    }
}

/// Complete authored callback accounting after its body returns, including its early-return paths.
pub(crate) fn observe_callback<R>(
    observer: Option<&TimingObserver>,
    begin: Option<Stamp>,
    kind: NativeCallbackKind,
    body: impl FnOnce() -> R,
) -> Result<R, DesktopError> {
    let result = body();
    if let (Some(observer), Some(begin)) = (observer, begin) {
        observer.callback(begin, kind, None)?;
    }
    Ok(result)
}

/// Delimit actual submission work and publish timing only after the complete boundary succeeds.
pub(crate) fn observe_submission<R>(
    observer: Option<&TimingObserver>,
    identity: FrameIdentity,
    submit: impl FnOnce() -> Result<R, DesktopError>,
) -> Result<R, DesktopError> {
    let begin = observer.map(TimingObserver::mark).transpose()?;
    let result = submit()?;
    if let (Some(observer), Some(begin)) = (observer, begin) {
        observer.submission(begin, identity)?;
    }
    Ok(result)
}
