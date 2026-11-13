// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit isolated purpose-typed helper transport before native adoption or launch.
//! Helper family, purpose codecs, replay and storage authority stay with their composing owners.
pub use crate::purpose_pipe::*;
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
    incident::DiagnosticRole,
    lifecycle::ProcessKey,
};
/// Pin the read-only purpose section through HANDLE_LIST and attribute deletion.
pub struct PurposePipeBootstrap {
    inner: LifecyclePipeBootstrap,
    key: PurposeKey,
}
impl PurposePipeBootstrap {
    /// Return the immutable admitted binding as facts only.
    pub const fn key(&self) -> PurposeKey {
        self.key
    }
    /// Return original signed QPC facts for the fixed child invocation.
    pub fn deadline(&self) -> Option<LifecyclePipeDeadline> {
        self.inner.deadline()
    }
    /// Require actual same-session pin provenance and the exact immutable binding.
    pub fn matches_enrollment(&self, enrollment: &PurposeEnrollment) -> bool {
        enrollment.key == self.key && enrollment.inner.matches(&self.inner.shared)
    }
    /// Borrow only the pinned read-only section for explicit native inheritance.
    pub fn borrow_inheritance(&self) -> LifecyclePipeInheritance<'_> {
        self.inner.borrow_inheritance()
    }
}
/// Retain the suspended helper handoff until exact native enrollment or cancellation.
pub struct PurposeEnrollment {
    inner: Enrollment,
    key: PurposeKey,
}
/// Preserve both actual native custody and the launch pin on rejected enrollment.
pub struct PurposeEnrollmentRejection {
    /// Closed typed rejection without decoded command contents.
    pub error: LifecyclePipeError,
    /// Original actual suspended process duplicate.
    pub process: OwnedHandle,
    /// Original pin, which remains responsible for cancellation on Drop.
    pub enrollment: PurposeEnrollment,
}
impl PurposeEnrollment {
    /// Return the trusted immutable helper binding without granting process authority.
    pub const fn key(&self) -> PurposeKey {
        self.key
    }
    /// Transfer the exact suspended helper pin before the registry resumes it.
    /// Trusted binding and registry key must come from the composing native owner.
    /// Rejection returns the original process duplicate and retained launch pin.
    pub fn enroll(
        mut self,
        trusted: PurposeKey,
        key: ProcessKey,
        process: OwnedHandle,
    ) -> Result<(), PurposeEnrollmentRejection> {
        match self.inner.bind(PeerBinding::Purpose(trusted), key, process, |handle| {
            PeerInput::Owned { handle, process: Some(key) }
        }) {
            Ok(()) => Ok(()),
            Err((error, process)) => {
                Err(PurposeEnrollmentRejection { error, process, enrollment: self })
            }
        }
    }
}
impl PurposePipeSession {
    /// Atomically transfer the original immutable bootstrap and suspended-process pin once.
    /// No additional capacity decision occurs between these two retained owners.
    pub fn take_bootstrap(
        &mut self,
    ) -> Result<Option<(PurposePipeBootstrap, PurposeEnrollment)>, LifecyclePipeError> {
        let Some((inner, enrollment)) = Enrollment::take_bootstrap(&self.control.inner)? else {
            return Ok(None);
        };
        let key = enrollment
            .binding()
            .and_then(PeerBinding::purpose)
            .ok_or(LifecyclePipeError::NotReady)?;
        Ok(Some((
            PurposePipeBootstrap { inner, key },
            PurposeEnrollment { inner: enrollment, key },
        )))
    }
}
/// Preserve original accounting custody when actual purpose admission rejects.
pub struct PurposePipeAdmissionRejection {
    /// Actual central decision or original clock/conversion cause.
    pub error: LifecyclePipeAdmissionError,
    /// The exact submitted accounting ownership, without native adoption.
    pub resources: crate::PurposePipeResources,
}
impl std::fmt::Debug for PurposePipeAdmissionRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PurposePipeAdmissionRejection")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}
impl std::fmt::Display for PurposePipeAdmissionRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.error, formatter)
    }
}
impl std::error::Error for PurposePipeAdmissionRejection {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}
/// Reserve the fixed child role before inheriting, mapping or connecting any handle.
pub struct PurposePipeClientPermit {
    original_deadline: Moment,
    session: Option<PurposePipeSession>,
}
impl PurposePipeClientPermit {
    /// Return the conservative local translation of the immutable original handshake.
    pub const fn original_deadline(&self) -> Moment {
        self.original_deadline
    }
    /// Transfer the exclusively inherited section to the already charged worker.
    /// Native decoding checks the previously latched fixed invocation role before pipe open.
    pub fn bind(mut self, section: InheritedLifecycleBootstrap) -> Option<PurposePipeSession> {
        let session = self.session.take()?;
        session.control.inner.bind_input(PeerInput::Inherited { section: section.into_raw() });
        Some(session)
    }
    /// Cancel an unused reservation while preserving its actual typed terminal outcome.
    pub fn cancel_unbound(mut self) -> Option<PurposePipeSession> {
        let session = self.session.take()?;
        release_input(&session.control.inner);
        Some(session)
    }
}
impl Drop for PurposePipeClientPermit {
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
    /// Admit one persistent server with a separate original finite handshake.
    /// The job and effective native scope must have no deadline so checked retirement survives expiry.
    /// Binding is trusted composing-owner input; facts alone grant no incident/storage authority.
    /// The session is the parent side of the binding's purpose and follows its direction rule.
    pub fn reserve_purpose_pipe_server(
        &self,
        scope: &Scope,
        spec: JobSpec,
        key: PurposeKey,
        original_handshake: Moment,
    ) -> Result<PurposePipeSession, LifecyclePipeAdmissionError> {
        lifecycle_pipe::validate_persistent_scope(&self.core, scope, spec)?;
        let (inner, completion) = lifecycle_pipe::admit(
            &self.core,
            scope,
            spec,
            Some(PeerBinding::Purpose(key)),
            original_handshake,
            Box::new(native::Lease::purpose_server(key)),
            Owner::Purpose,
        )
        .map_err(LifecyclePipeAdmissionError::Admission)?;
        crate::purpose_pipe::apply_loan_rule(&inner, key.purpose());
        Ok(PurposePipeSession {
            control: PurposePipeControl { inner, purpose: key.purpose(), side: Side::Parent },
            completion,
        })
    }
    /// Reserve the actual fixed helper invocation role before adopting its inherited bootstrap.
    /// The original counter frequency/deadline is translated once using the central clock owner.
    /// Decoded roles cannot select invocation privileges or renew this original budget.
    /// The session is the helper side of `expected` and follows its direction rule.
    pub fn reserve_purpose_pipe_client(
        &self,
        scope: &Scope,
        spec: JobSpec,
        expected: Purpose,
        original: LifecyclePipeDeadline,
        resources: crate::PurposePipeResources,
    ) -> Result<PurposePipeClientPermit, PurposePipeAdmissionRejection> {
        let preflight = lifecycle_pipe::validate_persistent_scope(&self.core, scope, spec)
            .and_then(|()| {
                if matches!(&resources, crate::PurposePipeResources::Symbol(_))
                    && expected != Purpose::Diagnostic(DiagnosticRole::Symbolize)
                {
                    return Err(LifecyclePipeAdmissionError::Admission(
                    sugarsubstitute_contracts::execution::AdmissionError::InvalidSymbolAdmission,
                ));
                }
                self.decode_lifecycle_deadline(original)
            });
        let until = match preflight {
            Ok(until) => until,
            Err(error) => return Err(PurposePipeAdmissionRejection { error, resources }),
        };
        let original_context = match &resources {
            crate::PurposePipeResources::Standard => None,
            crate::PurposePipeResources::Symbol(reservation) => {
                Some(reservation.original.as_ref().clone())
            }
        };
        let kind = match (&resources, &original_context) {
            (crate::PurposePipeResources::Standard, _) => crate::admission::RequestKind::Native,
            (crate::PurposePipeResources::Symbol(reservation), Some(original_context)) => {
                crate::admission::RequestKind::SymbolNative {
                    original: original_context,
                    handles: reservation.handles.clone(),
                }
            }
            (crate::PurposePipeResources::Symbol(_), None) => return Err(PurposePipeAdmissionRejection {
                error: LifecyclePipeAdmissionError::Admission(sugarsubstitute_contracts::execution::AdmissionError::InvalidSymbolAdmission),
                resources,
            }),
        };
        let (inner, completion) = lifecycle_pipe::admit_owned(
            &self.core,
            scope,
            spec,
            lifecycle_pipe::PipeAdmission {
                binding: None,
                handshake: until,
                owner: Owner::Purpose,
            },
            kind,
            resources,
            |resources, _context| {
                Ok(lifecycle_pipe::PreparedPipeLease {
                    lease: Box::new(native::Lease::purpose_client(
                        expected,
                        original,
                        match resources {
                            crate::PurposePipeResources::Standard => None,
                            crate::PurposePipeResources::Symbol(reservation) => Some(reservation),
                        },
                    )),
                    metadata: None,
                })
            },
        )
        .map_err(|(error, resources)| PurposePipeAdmissionRejection {
            error: LifecyclePipeAdmissionError::Admission(error),
            resources,
        })?;
        crate::purpose_pipe::apply_loan_rule(&inner, expected);
        Ok(PurposePipeClientPermit {
            original_deadline: until,
            session: Some(PurposePipeSession {
                control: PurposePipeControl { inner, purpose: expected, side: Side::Helper },
                completion,
            }),
        })
    }
}

#[cfg(test)]
#[path = "windows_purpose_pipe/tests.rs"]
mod tests;
