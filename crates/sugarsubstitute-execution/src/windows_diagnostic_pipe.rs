// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit isolated diagnostic helper transport before native adoption or launch.
//! Helper family, incident codec, replay and storage authority stay with their composing owners.
pub use crate::diagnostic_pipe::*;
pub use crate::windows_lifecycle_pipe::{
    InheritedLifecycleBootstrap, LifecyclePipeAdmissionError, LifecyclePipeInheritance,
};
use crate::{
    ExecutionClient,
    lifecycle_pipe::{self, Control, Owner, PeerBinding, PeerInput, enrollment::Enrollment},
    scope::lock,
    windows_lifecycle_pipe::{LifecyclePipeBootstrap, native},
};
use std::os::windows::io::OwnedHandle;
use sugarsubstitute_contracts::{
    execution::{JobSpec, Moment, Scope},
    incident::{DiagnosticKey, DiagnosticRole},
    lifecycle::ProcessKey,
};
/// Pin the read-only diagnostic section through HANDLE_LIST and attribute deletion.
pub struct DiagnosticPipeBootstrap {
    inner: LifecyclePipeBootstrap,
    key: DiagnosticKey,
}
impl DiagnosticPipeBootstrap {
    /// Return the immutable admitted binding as facts only.
    pub const fn key(&self) -> DiagnosticKey {
        self.key
    }
    /// Return original signed QPC facts for the fixed child invocation.
    pub fn deadline(&self) -> Option<LifecyclePipeDeadline> {
        self.inner.deadline()
    }
    /// Require actual same-session pin provenance and the exact immutable binding.
    pub fn matches_enrollment(&self, enrollment: &DiagnosticEnrollment) -> bool {
        enrollment.key == self.key && enrollment.inner.matches(&self.inner.shared)
    }
    /// Borrow only the pinned read-only section for explicit native inheritance.
    pub fn borrow_inheritance(&self) -> LifecyclePipeInheritance<'_> {
        self.inner.borrow_inheritance()
    }
}
/// Retain the suspended helper handoff until exact native enrollment or cancellation.
pub struct DiagnosticEnrollment {
    inner: Enrollment,
    key: DiagnosticKey,
}
/// Preserve both actual native custody and the launch pin on rejected enrollment.
pub struct DiagnosticEnrollmentRejection {
    /// Closed typed rejection without decoded command contents.
    pub error: LifecyclePipeError,
    /// Original actual suspended process duplicate.
    pub process: OwnedHandle,
    /// Original pin, which remains responsible for cancellation on Drop.
    pub enrollment: DiagnosticEnrollment,
}
impl DiagnosticEnrollment {
    /// Return the trusted immutable helper binding without granting process authority.
    pub const fn key(&self) -> DiagnosticKey {
        self.key
    }
    /// Transfer the exact suspended helper pin before the registry resumes it.
    /// Trusted binding and registry key must come from the composing native owner.
    /// Rejection returns the original process duplicate and retained launch pin.
    pub fn enroll(
        mut self,
        trusted: DiagnosticKey,
        key: ProcessKey,
        process: OwnedHandle,
    ) -> Result<(), DiagnosticEnrollmentRejection> {
        match self.inner.bind(PeerBinding::Diagnostic(trusted), key, process, |handle| {
            PeerInput::Owned { handle, process: Some(key) }
        }) {
            Ok(()) => Ok(()),
            Err((error, process)) => {
                Err(DiagnosticEnrollmentRejection { error, process, enrollment: self })
            }
        }
    }
}
impl DiagnosticPipeSession {
    /// Atomically transfer the original immutable bootstrap and suspended-process pin once.
    /// No additional capacity decision occurs between these two retained owners.
    pub fn take_bootstrap(
        &mut self,
    ) -> Result<Option<(DiagnosticPipeBootstrap, DiagnosticEnrollment)>, LifecyclePipeError> {
        let Some((inner, enrollment)) = Enrollment::take_bootstrap(&self.control.inner)? else {
            return Ok(None);
        };
        let key = enrollment
            .binding()
            .and_then(PeerBinding::diagnostic)
            .ok_or(LifecyclePipeError::NotReady)?;
        Ok(Some((
            DiagnosticPipeBootstrap { inner, key },
            DiagnosticEnrollment { inner: enrollment, key },
        )))
    }
}
/// Reserve the fixed child role before inheriting, mapping or connecting any handle.
pub struct DiagnosticPipeClientPermit {
    original_deadline: Moment,
    session: Option<DiagnosticPipeSession>,
}
impl DiagnosticPipeClientPermit {
    /// Return the conservative local translation of the immutable original handshake.
    pub const fn original_deadline(&self) -> Moment {
        self.original_deadline
    }
    /// Transfer the exclusively inherited section to the already charged worker.
    /// Native decoding checks the previously latched fixed invocation role before pipe open.
    pub fn bind(mut self, section: InheritedLifecycleBootstrap) -> Option<DiagnosticPipeSession> {
        let session = self.session.take()?;
        session.control.inner.bind_input(PeerInput::Inherited { section: section.into_raw() });
        Some(session)
    }
    /// Cancel an unused reservation while preserving its actual typed terminal outcome.
    pub fn cancel_unbound(mut self) -> Option<DiagnosticPipeSession> {
        let session = self.session.take()?;
        release_input(&session.control.inner);
        Some(session)
    }
}
impl Drop for DiagnosticPipeClientPermit {
    fn drop(&mut self) {
        if let Some(session) = self.session.take() {
            release_input(&session.control.inner)
        }
    }
}
fn release_input(control: &Control) {
    {
        lock(&control.shared.state).input_pin = false;
    }
    control.close();
}
impl ExecutionClient {
    fn diagnostic_persistent_scope(
        &self,
        scope: &Scope,
        spec: JobSpec,
    ) -> Result<(), LifecyclePipeAdmissionError> {
        if spec.deadline.is_some() {
            return Err(LifecyclePipeAdmissionError::ConflictingDeadline);
        }
        let state = lock(&self.core.state);
        if state.scopes.get(&scope.id()).is_some_and(|s| s.deadline.is_some()) {
            return Err(LifecyclePipeAdmissionError::ConflictingDeadline);
        }
        Ok(())
    }
    /// Admit one persistent server with a separate original finite handshake.
    /// The job and effective native scope must have no deadline so checked retirement survives expiry.
    /// Binding is trusted composing-owner input; facts alone grant no incident/storage authority.
    pub fn reserve_diagnostic_pipe_server(
        &self,
        scope: &Scope,
        spec: JobSpec,
        key: DiagnosticKey,
        original_handshake: Moment,
    ) -> Result<DiagnosticPipeSession, LifecyclePipeAdmissionError> {
        self.diagnostic_persistent_scope(scope, spec)?;
        let (inner, completion) = lifecycle_pipe::admit(
            &self.core,
            scope,
            spec,
            Some(PeerBinding::Diagnostic(key)),
            original_handshake,
            Box::new(native::Lease::diagnostic_server(key)),
            Owner::Diagnostic,
        )
        .map_err(LifecyclePipeAdmissionError::Admission)?;
        Ok(DiagnosticPipeSession { control: DiagnosticPipeControl { inner }, completion })
    }
    /// Reserve the actual fixed helper invocation role before adopting its inherited bootstrap.
    /// The original counter frequency/deadline is translated once using the central clock owner.
    /// Decoded roles cannot select invocation privileges or renew this original budget.
    pub fn reserve_diagnostic_pipe_client(
        &self,
        scope: &Scope,
        spec: JobSpec,
        expected_role: DiagnosticRole,
        original: LifecyclePipeDeadline,
    ) -> Result<DiagnosticPipeClientPermit, LifecyclePipeAdmissionError> {
        self.diagnostic_persistent_scope(scope, spec)?;
        let until = self.decode_lifecycle_deadline(original)?;
        let (inner, completion) = lifecycle_pipe::admit(
            &self.core,
            scope,
            spec,
            None,
            until,
            Box::new(native::Lease::diagnostic_client(expected_role, original)),
            Owner::Diagnostic,
        )
        .map_err(LifecyclePipeAdmissionError::Admission)?;
        Ok(DiagnosticPipeClientPermit {
            original_deadline: until,
            session: Some(DiagnosticPipeSession {
                control: DiagnosticPipeControl { inner },
                completion,
            }),
        })
    }
}

#[cfg(test)]
#[path = "windows_diagnostic_pipe/tests.rs"]
mod tests;
