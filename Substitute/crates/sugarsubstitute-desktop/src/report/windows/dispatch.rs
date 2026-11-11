// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Loan one charged surface owner outside Tao callbacks and coalesce only its native window facts.
use super::*;
use std::cell::Cell;
use sugarsubstitute_contracts::execution::MemoryClaim;
/// These UI-thread facts neither create a host acknowledgement nor prove native retirement.
pub(super) struct Deferred {
    window: Cell<Option<tao::window::WindowId>>,
    close: Cell<Option<Instant>>,
    destroyed: Cell<Option<Instant>>,
    layout: Cell<bool>,
    pub(super) cancel: Rc<ModalCancellation>,
    failure: Cell<Option<ReportSurfaceError>>,
}
impl Deferred {
    fn new(funding: Rc<DispatchFunding>) -> Self {
        Self {
            window: Cell::new(None),
            close: Cell::new(None),
            destroyed: Cell::new(None),
            layout: Cell::new(false),
            cancel: Rc::new(ModalCancellation::new(funding)),
            failure: Cell::new(None),
        }
    }
    pub(super) fn bind(&self, window: Option<tao::window::WindowId>) {
        self.window.set(window);
    }
    /// Coalesce native facts only for the actual bound window; the caller unwraps Tao's envelope.
    pub(super) fn capture(&self, window_id: tao::window::WindowId, event: &WindowEvent<'_>) {
        if self.window.get() == Some(window_id) {
            match event {
                WindowEvent::CloseRequested => {
                    self.close.set(Some(self.close.get().unwrap_or_else(Instant::now)));
                    self.cancel.set(true);
                }
                WindowEvent::Destroyed => {
                    self.destroyed.set(Some(self.destroyed.get().unwrap_or_else(Instant::now)));
                    self.cancel.set(true);
                }
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                    self.layout.set(true)
                }
                _ => {}
            }
        }
    }
    pub(super) fn fail(&self, cause: ReportSurfaceError) {
        let previous = self.failure.take();
        self.failure.set(Some(previous.unwrap_or(cause)));
        self.cancel.set(true);
    }
}
pub(super) struct Surface<T> {
    pub(super) session: session::Session,
    pub(super) content: T,
    pub(super) apartment: Option<crate::windows_ui_apartment::UiApartment>,
    pub(super) expired: bool,
    pub(super) modal_wake_queued: bool,
    _funding: Rc<DispatchFunding>,
}
/// The same-thread Cell owns the sole actual Box; a loan leaves no reentrant content access.
pub(super) struct SurfaceOwner<T> {
    slot: Cell<Option<Box<Surface<T>>>>,
    pub(super) deferred: Deferred,
    funding: Rc<DispatchFunding>,
}
pub(super) struct SurfaceLoan<'a, T> {
    owner: &'a SurfaceOwner<T>,
    value: Option<Box<Surface<T>>>,
}
/// Bound actual owner/surface/loan and transfer metadata, Rc headers and closure capture.
/// All holders share the one dispatch reservation transferred from surface options.
pub(super) const fn storage_bytes<T>() -> usize {
    size_of::<SurfaceOwner<T>>()
        + size_of::<Surface<T>>()
        + 12 * size_of::<usize>()
        + size_of::<DispatchFunding>()
        + size_of::<ModalCancellation>()
        + size_of::<FundedContent<T>>()
        + size_of::<FundedFailure>()
        + size_of::<SurfaceLoan<'_, T>>()
}
impl<T> SurfaceOwner<T> {
    /// Both public entry paths check storage_bytes before start; every actual holder shares funding.
    pub(super) fn new(session: session::Session, content: T, storage: MemoryClaim) -> Rc<Self> {
        let funding = Rc::new(DispatchFunding { _storage: storage });
        Rc::new(Self {
            slot: Cell::new(Some(Box::new(Surface {
                session,
                content,
                apartment: None,
                expired: false,
                modal_wake_queued: false,
                _funding: funding.clone(),
            }))),
            deferred: Deferred::new(funding.clone()),
            funding,
        })
    }
    /// Content is dropped before its funding; an existing unresolved finisher retains both together.
    pub(super) fn fund_content(&self, content: T) -> FundedContent<T> {
        FundedContent { content, funding: self.funding.clone() }
    }
    pub(super) fn fund_failure(&self, cause: ReportSurfaceError) -> ReportSurfaceError {
        self.funding.failure(cause)
    }
    pub(super) fn loan(&self) -> Option<SurfaceLoan<'_, T>> {
        self.slot.take().map(|value| SurfaceLoan { owner: self, value: Some(value) })
    }
    pub(super) fn take_final(&self) -> Result<Box<Surface<T>>, ReportSurfaceError> {
        self.slot.take().ok_or_else(|| ReportSurfaceError::rejected("report_dispatch_still_loaned"))
    }
}
impl<T: ReportSessionContent> SurfaceOwner<T> {
    /// The existing finisher retains content and funding together on uncertain final disposal.
    pub(super) fn finish_content(
        &self,
        content: T,
        after_join: impl FnOnce() -> Result<(), crate::DesktopError>,
        failure: &mut Option<crate::DesktopError>,
    ) {
        crate::shutdown::finish_after_native(
            self.fund_content(content),
            |wrapped| {
                wrapped.content.finish_shutdown().map_err(|cause| {
                    crate::DesktopError::new("finish_report_shutdown", wrapped.fund_failure(cause))
                })
            },
            || {
                after_join().map_err(|cause| {
                    crate::DesktopError::new(
                        "report_native_finalization",
                        self.fund_failure(cause.into()),
                    )
                })
            },
            failure,
        );
    }
    /// The existing native loop remains installed; this method is called only by with_msg_hook.
    pub(super) fn message_turn(&self) {
        let Some(mut loan) = self.loan() else { return };
        let state = match loan.state() {
            Ok(state) => state,
            Err(cause) => {
                self.deferred.fail(cause);
                return;
            }
        };
        self.apply_deferred(state);
        if state.session.prepared_save.is_some() && !state.session.state.closing() {
            state.modal_wake_queued = false;
            if let Err(cause) =
                state.session.run_prepared_save(&mut state.content, self.deferred.cancel.clone())
            {
                state.session.fail(&mut state.content, cause);
            }
            self.apply_deferred(state);
        }
    }
    pub(super) fn apply_deferred(&self, state: &mut Surface<T>) {
        if let Some(cause) = self.deferred.failure.take() {
            state.session.fail(&mut state.content, cause);
        }
        if let Some(observed) = self.deferred.destroyed.take() {
            state.session.destroyed = true;
            if !state.session.state.closing() {
                state.session.fail_at(
                    &mut state.content,
                    ReportSurfaceError::rejected("report_parent_destroyed_during_modal"),
                    observed,
                );
            }
        }
        if let Some(observed) = self.deferred.close.take()
            && !state.session.state.closing()
        {
            let input = Input {
                binding: state.session.state.binding(),
                intent: super::super::state::Intent::Action(ReportAction::Dismiss),
            };
            if let Err(cause) = state.session.input(&mut state.content, input) {
                state.session.fail_at(&mut state.content, cause, observed);
            }
        }
        if self.deferred.layout.replace(false)
            && !state.session.state.closing()
            && let Err(cause) = state.session.controls.as_mut().map_or(Ok(()), |c| c.layout())
        {
            state.session.fail(&mut state.content, cause);
        }
        self.deferred
            .cancel
            .set(self.deferred.close.get().is_some() || self.deferred.destroyed.get().is_some());
    }
}
impl<T> SurfaceLoan<'_, T> {
    pub(super) fn state(&mut self) -> Result<&mut Surface<T>, ReportSurfaceError> {
        self.value
            .as_deref_mut()
            .ok_or_else(|| ReportSurfaceError::rejected("report_dispatch_loan_empty"))
    }
}
impl<T> Drop for SurfaceLoan<'_, T> {
    fn drop(&mut self) {
        // Only loan() removes the private slot, and no successor can loan it until this restore.
        self.owner.slot.set(self.value.take());
    }
}
#[cfg(test)]
mod tests;

impl<T> Drop for SurfaceOwner<T> {
    fn drop(&mut self) {
        // Only explicit post-loop take_final transfers an observed settled box to checked finish.
        if let Some(state) = self.slot.take() {
            std::mem::forget(state);
        }
    }
}

/// One real reservation shared only by the actual authored allocations it backs.
struct DispatchFunding {
    _storage: MemoryClaim,
}
impl DispatchFunding {
    fn failure(self: &Rc<Self>, cause: ReportSurfaceError) -> ReportSurfaceError {
        ReportSurfaceError::new(cause.operation(), FundedFailure { cause, _funding: self.clone() })
    }
}
/// The last field pins funding while content or an escaping failure retains real custody.
pub(super) struct FundedContent<T> {
    pub(super) content: T,
    funding: Rc<DispatchFunding>,
}
impl<T> FundedContent<T> {
    pub(super) fn fund_failure(&self, cause: ReportSurfaceError) -> ReportSurfaceError {
        self.funding.failure(cause)
    }
}
struct FundedFailure {
    cause: ReportSurfaceError,
    _funding: Rc<DispatchFunding>,
}
impl std::fmt::Debug for FundedFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FundedReportFailure")
    }
}
impl std::fmt::Display for FundedFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Native report failure retains original dispatch custody")
    }
}
impl std::error::Error for FundedFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}
/// Clones carry only same-thread cancellation and its existing reservation, never process authority.
pub(super) struct ModalCancellation {
    requested: Cell<bool>,
    _funding: Rc<DispatchFunding>,
}
impl ModalCancellation {
    fn new(funding: Rc<DispatchFunding>) -> Self {
        Self { requested: Cell::new(false), _funding: funding }
    }
    pub(super) fn get(&self) -> bool {
        self.requested.get()
    }
    pub(super) fn set(&self, requested: bool) {
        self.requested.set(requested);
    }
    #[cfg(test)]
    pub(super) fn fixture(
        lease: &sugarsubstitute_contracts::execution::MemoryLease,
    ) -> Result<Rc<Self>, ReportSurfaceError> {
        let storage = lease
            .claim(256)
            .map_err(|cause| ReportSurfaceError::new("report_fixture_cancel_credit", cause))?;
        Ok(Rc::new(Self::new(Rc::new(DispatchFunding { _storage: storage }))))
    }
}
