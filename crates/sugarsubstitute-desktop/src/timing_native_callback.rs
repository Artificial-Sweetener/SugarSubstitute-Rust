// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe authored native subclass bodies while retaining timing ownership across final destruction.

use crate::{
    runtime::NativeAction,
    timing::{NativeCallbackKind, NativeTickInterval, Stamp, TimingObserver},
};
use tao::event_loop::EventLoopProxy;

/// Retain a cheap UI-only sink and proxy; no collector borrow survives any native call.
pub(crate) struct NativeCallbackObservation {
    observer: Option<TimingObserver>,
    proxy: EventLoopProxy<NativeAction>,
    begin: Option<Stamp>,
    kind: NativeCallbackKind,
    delegated: Option<NativeTickInterval>,
}
impl NativeCallbackObservation {
    /// Mark callback entry while retaining an independent sink across final callback-data release.
    pub(crate) fn new(
        observer: Option<TimingObserver>,
        proxy: EventLoopProxy<NativeAction>,
        kind: NativeCallbackKind,
    ) -> Self {
        let begin = observer.as_ref().and_then(|sink| match sink.mark() {
            Ok(begin) => Some(begin),
            Err(cause) => {
                retain_and_wake(sink, &proxy, cause);
                None
            }
        });
        Self { observer, proxy, begin, kind, delegated: None }
    }
    /// Exclude exactly the SC_MOVE default-procedure call; inner callbacks remain independently observed.
    pub(crate) fn delegated<R>(&mut self, call: impl FnOnce() -> R) -> R {
        let begin = self.observer.as_ref().and_then(|sink| match sink.mark() {
            Ok(mark) => Some(mark),
            Err(cause) => {
                retain_and_wake(sink, &self.proxy, cause);
                None
            }
        });
        let result = call();
        if let Some(sink) = &self.observer {
            match sink.mark() {
                Ok(end) => {
                    if let Some(begin) = begin {
                        self.delegated = Some(NativeTickInterval {
                            begin_ticks: begin.ticks,
                            end_ticks: end.ticks,
                        });
                    }
                }
                Err(cause) => sink.fail(cause),
            }
        }
        result
    }
    /// Call after forwarding returns, including after WM_NCDESTROY consumes its callback Rc.
    pub(crate) fn finish(self) {
        if let (Some(sink), Some(begin)) = (&self.observer, self.begin)
            && let Err(cause) = sink.callback(begin, self.kind, self.delegated)
        {
            sink.fail(cause);
        }
        if let Some(sink) = &self.observer
            && sink.request_failure_wake()
            && let Err(cause) = self.proxy.send_event(NativeAction::Completions)
        {
            sink.fail(crate::DesktopError::new("wake_native_timing_failure", cause));
        }
    }
}

/// Retain a clock failure and signal one batch before potentially modal native forwarding.
fn retain_and_wake(
    sink: &TimingObserver,
    proxy: &EventLoopProxy<NativeAction>,
    cause: crate::DesktopError,
) {
    sink.fail(cause);
    if sink.request_failure_wake()
        && let Err(cause) = proxy.send_event(NativeAction::Completions)
    {
        sink.fail(crate::DesktopError::new("wake_native_timing_failure", cause));
    }
}
