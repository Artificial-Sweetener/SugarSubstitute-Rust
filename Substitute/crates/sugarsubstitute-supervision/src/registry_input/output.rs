// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind exact captured output to one trusted owned launch without closing rejected sources.
use super::*;
use crate::FamilyBinding;
use sugarsubstitute_execution::windows_output::OutputIdentity;
use sugarsubstitute_execution::windows_output::{
    WindowsChildStdio, WindowsOutputCancellation, WindowsOutputControl, WindowsOutputError,
};

/// Preserve both original owned inputs when output association is rejected.
/// Dropping this rejection drops its input normally; a returned unattached stdio
/// pin only unpins its source. The caller retains the original output control.
pub struct RejectedOutputLaunch {
    /// Original launch input, including any previously accepted pins.
    pub input: Box<WindowsLaunchInput>,
    /// Original stdio artifact; it was not attached or explicitly cancelled.
    pub stdio: WindowsChildStdio,
    /// Typed association failure, with redacted Display/Debug.
    pub error: RegistryInputError,
}
impl fmt::Debug for RejectedOutputLaunch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RejectedOutputLaunch").field("error", &self.error).finish_non_exhaustive()
    }
}
impl fmt::Display for RejectedOutputLaunch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}
impl std::error::Error for RejectedOutputLaunch {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}
pub(crate) struct OutputLaunch {
    pub(crate) binding: FamilyBinding,
    pub(crate) identity: OutputIdentity,
    pub(crate) stdio: OutputPin,
}
impl OutputLaunch {
    pub(crate) fn validates(&self, binding: FamilyBinding) -> bool {
        self.binding == binding
            && binding_matches(binding, self.identity)
            && self.stdio.identity() == self.identity
    }
    pub(crate) fn request_close(&self) {
        self.stdio.request_close();
    }
}
pub(crate) fn binding_matches(binding: FamilyBinding, identity: OutputIdentity) -> bool {
    match (binding, identity) {
        (FamilyBinding::Managed(expected), OutputIdentity::Managed(actual)) => expected == actual,
        (FamilyBinding::Desktop(expected), OutputIdentity::Desktop(actual)) => expected == actual,
        _ => false,
    }
}
impl WindowsLaunchInput {
    /// Attach one exact output session to an owned desktop or managed launch.
    /// Managed output matches its exact resource generation; desktop output matches
    /// its exact Run generation. Session Arc provenance
    /// is checked independently of these matching facts before any admission.
    /// Fixture events are mutually exclusive. Lifecycle plus stdio uses four
    /// handles; captured lifecycle includes two source pins for a total of six.
    /// Rejection returns both inputs without closing a foreign
    /// output. Accepted input Drop requests output retirement if never transferred.
    /// The caller retains the original output session/scope with its identity;
    /// process/family settlement does not establish output EOF or retirement.
    pub fn with_owned_output(
        mut self,
        binding: FamilyBinding,
        identity: OutputIdentity,
        control: &WindowsOutputControl,
        stdio: WindowsChildStdio,
    ) -> Result<Self, RejectedOutputLaunch> {
        let error = association_error(
            &self,
            binding,
            identity,
            stdio.identity(),
            control.snapshot().identity,
            control.matches_child_stdio(&stdio),
        );
        if let Some(error) = error {
            return Err(RejectedOutputLaunch { input: Box::new(self), stdio, error });
        }
        self.output = Some(OutputLaunch { binding, identity, stdio: OutputPin::Native(stdio) });
        Ok(self)
    }
}
pub(crate) fn association_error(
    input: &WindowsLaunchInput,
    binding: FamilyBinding,
    identity: OutputIdentity,
    pin_identity: OutputIdentity,
    control_identity: OutputIdentity,
    same_session: bool,
) -> Option<RegistryInputError> {
    if input.output.is_some() || input.fixture_events.is_some() || input.diagnostic.is_some() {
        Some(RegistryInputError::OutputConflict)
    } else if !binding_matches(binding, identity)
        || input
            .lifecycle
            .as_ref()
            .is_some_and(|launch| binding != FamilyBinding::Desktop(launch.run))
    {
        Some(RegistryInputError::OutputBinding)
    } else if pin_identity != identity || control_identity != identity || !same_session {
        Some(RegistryInputError::OutputProvenance)
    } else {
        None
    }
}
impl Drop for WindowsLaunchInput {
    fn drop(&mut self) {
        if let Some(output) = &self.output {
            output.request_close();
        }
    }
}

/// Private native artifact boundary; test fakes replace only external pin effects.
pub(crate) enum OutputPin {
    Native(WindowsChildStdio),
    #[cfg(test)]
    Simulated(fake::Pin),
}
pub(crate) enum OutputCancellation {
    Native(WindowsOutputCancellation),
    #[cfg(test)]
    Simulated(std::sync::Arc<fake::Source>),
}
impl OutputPin {
    pub(crate) fn identity(&self) -> OutputIdentity {
        match self {
            Self::Native(pin) => pin.identity(),
            #[cfg(test)]
            Self::Simulated(pin) => pin.0.identity,
        }
    }
    pub(crate) fn handles(
        &self,
        identity: OutputIdentity,
    ) -> Result<[usize; 3], WindowsOutputError> {
        match self {
            Self::Native(pin) => {
                use std::os::windows::io::AsRawHandle;
                let view = pin.borrow_inheritance(identity)?;
                Ok([view.stdin, view.stdout, view.stderr]
                    .map(|handle| handle.as_raw_handle() as usize))
            }
            #[cfg(test)]
            Self::Simulated(pin) => pin.0.handles(identity),
        }
    }
    pub(crate) fn request_close(&self) {
        match self {
            Self::Native(pin) => pin.request_close(),
            #[cfg(test)]
            Self::Simulated(pin) => pin.0.request_close(),
        }
    }
    pub(crate) fn release(self) -> OutputCancellation {
        match self {
            Self::Native(pin) => OutputCancellation::Native(pin.release_after_attributes()),
            #[cfg(test)]
            Self::Simulated(pin) => {
                let source = pin.0.clone();
                drop(pin);
                OutputCancellation::Simulated(source)
            }
        }
    }
}
impl OutputCancellation {
    pub(crate) fn request_close(&self) {
        match self {
            Self::Native(cancel) => cancel.request_close(),
            #[cfg(test)]
            Self::Simulated(source) => source.request_close(),
        }
    }
}
#[cfg(test)]
pub(crate) mod fake {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    pub(crate) struct Source {
        pub(crate) identity: OutputIdentity,
        pub(crate) requests: AtomicUsize,
        pub(crate) releases: AtomicUsize,
        pub(crate) closed: AtomicBool,
    }
    impl Source {
        pub(crate) fn new(identity: OutputIdentity) -> Arc<Self> {
            Arc::new(Self {
                identity,
                requests: AtomicUsize::new(0),
                releases: AtomicUsize::new(0),
                closed: AtomicBool::new(false),
            })
        }
        pub(super) fn handles(
            &self,
            identity: OutputIdentity,
        ) -> Result<[usize; 3], WindowsOutputError> {
            if identity != self.identity {
                return Err(WindowsOutputError::ForeignIdentity);
            }
            if self.closed.load(Ordering::Acquire) {
                return Err(WindowsOutputError::Closed);
            }
            Ok([71, 72, 73])
        }
        pub(super) fn request_close(&self) {
            self.requests.fetch_add(1, Ordering::AcqRel);
            self.closed.store(true, Ordering::Release);
        }
    }
    pub(crate) struct Pin(pub(crate) Arc<Source>);
    impl Drop for Pin {
        fn drop(&mut self) {
            self.0.releases.fetch_add(1, Ordering::AcqRel);
        }
    }
}
