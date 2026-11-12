// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Restore the same charged input and complete view state after every finite worker or unwind.
use super::{
    client::NativeInput,
    view::{PreparedView, ViewReceiver},
};
use std::sync::{Arc, Mutex};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_crash::diagnostic_client::*;
pub(super) const SOURCE_BYTES: u64 = 32 * 1024;
struct State {
    receiver: ViewReceiver,
    input: Option<NativeInput>,
    prepared: Option<PreparedView>,
    wire: Option<super::view::ViewFailure>,
    error_original: Option<MemoryLease>,
    rejected: Option<ReceivedDiagnosticRequest>,
    until: Option<(DiagnosticDeadline, Moment)>,
}
struct Shared {
    state: Mutex<Option<Box<State>>>,
    owner: ExecutionId,
    _claim: MemoryClaim,
}
pub(super) struct ViewSource {
    shared: Arc<Shared>,
    _outside: MemoryLease,
}
const _: () = assert!(
    size_of::<Shared>() + size_of::<State>() + 4 * size_of::<usize>() < SOURCE_BYTES as usize
);
impl ViewSource {
    pub(super) fn new(context: &JobContext) -> Result<Self, MemoryClaimError> {
        let claim = context.memory().claim(SOURCE_BYTES)?;
        Ok(Self {
            shared: Arc::new(Shared {
                state: Mutex::new(Some(Box::new(State {
                    receiver: ViewReceiver::new(context),
                    input: None,
                    prepared: None,
                    wire: None,
                    error_original: None,
                    rejected: None,
                    until: None,
                }))),
                owner: context.owner_id(),
                _claim: claim,
            }),
            _outside: context.memory().clone(),
        })
    }
    pub(super) fn alias(&self) -> Self {
        Self { shared: self.shared.clone(), _outside: self._outside.clone() }
    }
    /// Occupancy rejection leaves the complete original input unchanged.
    pub(super) fn install(&self, input: &mut Option<NativeInput>) -> Result<(), ViewSourceIssue> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let state = state.as_mut().ok_or(ViewSourceIssue::Busy)?;
        if state.input.is_some()
            || state.prepared.is_some()
            || state.wire.is_some()
            || state.rejected.is_some()
            || state.receiver.failure().is_some()
        {
            return Err(ViewSourceIssue::Busy);
        }
        state.input = Some(input.take().ok_or(ViewSourceIssue::Busy)?);
        Ok(())
    }
    /// Decode outside the source mutex; Drop restores all partial owner state on unwind.
    pub(super) fn process(
        &self,
        context: &JobContext,
        decode: impl FnOnce(
            DiagnosticDeadline,
        ) -> Result<
            Moment,
            sugarsubstitute_execution::windows::NativeObservationError,
        >,
        now: impl FnOnce() -> Result<Moment, sugarsubstitute_execution::windows::NativeObservationError>,
    ) -> Result<(), ViewSourceIssue> {
        if context.owner_id() != self.shared.owner {
            return Err(ViewSourceIssue::Identity);
        }
        let state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
            .ok_or(ViewSourceIssue::Busy)?;
        let mut loan = Loan { source: self, state: Some(state) };
        let state = loan.state.as_mut().ok_or(ViewSourceIssue::Busy)?;
        let input = state.input.take().ok_or(ViewSourceIssue::Busy)?;
        let request =
            match ReceivedDiagnosticRequest::from_native(input.peer, input.key, input.frame) {
                Ok(request) => request,
                Err(cause) => {
                    state.wire = Some(super::view::ViewFailure::Wire(cause.error));
                    state.error_original = Some(context.memory().clone());
                    state.input =
                        Some(NativeInput { peer: input.peer, key: input.key, frame: cause.frame });
                    return Err(ViewSourceIssue::Wire);
                }
            };
        let controls = (|| {
            let frame = request.decode().map_err(super::view::ViewFailure::Wire)?;
            let now = now().map_err(super::view::ViewFailure::Clock)?;
            if context.deadline().is_none_or(|until| now >= until)
                || context.cancellation_requested()
            {
                return Err(super::view::ViewFailure::Wire(DiagnosticWireError::Time));
            }
            if let Some(deadline) = frame.context.deadline {
                let until = match state.until {
                    Some((original, until)) if original == deadline => until,
                    Some(_) => {
                        return Err(super::view::ViewFailure::Wire(DiagnosticWireError::Stale));
                    }
                    None => decode(deadline).map_err(super::view::ViewFailure::Clock)?.min(
                        context
                            .deadline()
                            .ok_or(super::view::ViewFailure::Wire(DiagnosticWireError::Time))?,
                    ),
                };
                if now >= until || context.cancellation_requested() {
                    return Err(super::view::ViewFailure::Wire(DiagnosticWireError::Time));
                }
                state.until = Some((deadline, until));
            }
            Ok(())
        })();
        if let Err(cause) = controls {
            state.wire = Some(cause);
            state.error_original = Some(context.memory().clone());
            state.rejected = Some(request);
            return Err(ViewSourceIssue::Wire);
        }
        let mut request = Some(request);
        state.prepared =
            Some(state.receiver.apply(&mut request, context).map_err(|_| ViewSourceIssue::View)?);
        if !matches!(state.prepared, Some(PreparedView::Partial)) {
            state.until = None;
        }
        Ok(())
    }
    /// Only matched worker completion permits consuming prepared immutable text.
    pub(super) fn take_prepared(&self) -> Option<PreparedView> {
        self.shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_mut()
            .and_then(|state| state.prepared.take())
    }
    /// Deliver the exact semantic cause once while retaining rejected frame custody.
    pub(super) fn take_failure(&self) -> Option<super::view::OwnedViewFailure> {
        self.shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_mut()
            .and_then(|state| {
                state.receiver.take_failure().or_else(|| {
                    Some(super::view::OwnedViewFailure::from_source(
                        state.wire.take()?,
                        state.error_original.take()?,
                    ))
                })
            })
    }
    /// Retain only the original finite reservation credit, without borrowing the input/frame cell.
    pub(super) fn retained_memory(&self) -> MemoryLease {
        self._outside.clone()
    }
    /// Observe the one conservatively decoded stream cutoff without sampling clocks again.
    pub(super) fn input_until(&self) -> Option<Moment> {
        self.shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .and_then(|state| state.until.map(|(_, until)| until))
    }
    pub(super) fn presented(
        &self,
        binding: sugarsubstitute_contracts::incident::ReportBinding,
    ) -> Result<(), ViewSourceIssue> {
        let mut state = self.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state
            .as_mut()
            .ok_or(ViewSourceIssue::Busy)?
            .receiver
            .presented(binding)
            .map_err(|_| ViewSourceIssue::Identity)
    }
}
struct Loan<'a> {
    source: &'a ViewSource,
    state: Option<Box<State>>,
}
impl Drop for Loan<'_> {
    fn drop(&mut self) {
        let mut slot =
            self.source.shared.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        *slot = self.state.take();
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ViewSourceIssue {
    Busy,
    Identity,
    Wire,
    View,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::production::incident_host::tests::admitted_value;
    #[test]
    fn missing_original_frame_restores_same_funded_cell_and_last_alias_keeps_original_charge()
    -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        admitted_value(crate::production::incident_host::CONTROLLER_BYTES, |context| {
            let source = ViewSource::new(&context)?;
            let pointer = source
                .shared
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_ref()
                .map(|state| std::ptr::from_ref(&**state));
            assert_eq!(
                source.process(
                    &context,
                    |_| Ok(Moment::from_nanos(0)),
                    || Ok(Moment::from_nanos(0))
                ),
                Err(ViewSourceIssue::Busy)
            );
            assert_eq!(
                source
                    .shared
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .as_ref()
                    .map(|state| std::ptr::from_ref(&**state)),
                pointer
            );
            let alias = source.alias();
            drop(source);
            Ok(alias)
        })
    }
}
