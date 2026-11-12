// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt startup only from the original authenticated Root recovery session.
//! Original bytes and their charged assembly remain process-local; decoded grant
//! facts, bare buffers or a caller-built JobContext cannot construct this source.
mod source;
mod storage;
use super::*;
use crate::{
    coordinator_entry::{PreparedStartupSnapshot, startup_launch_payload},
    lifecycle_pipe::{Owner, SharedRef},
    scope::lock,
};
pub(crate) use source::Source;
use std::sync::Arc;
pub(crate) use storage::SourceRef;
use sugarsubstitute_contracts::{
    execution::{
        AdmissionError, CounterDeadline, ExecutionPort, JobClass, JobContext, JobSpec, Moment,
        Operation, OwnedJob, Scope,
    },
    incident::DurableAcknowledgement,
    recovery_supervisor::{RecoveryGrantKey, wire::*},
};

/// Preserve the finite source category while retaining each original provider cause.
#[derive(Debug)]
pub enum RecoveryStartupError {
    /// The real executing owner or memory/cancellation association rejected admission.
    Admission(AdmissionError),
    /// The actual authenticated transport is stopped or belongs to another owner.
    Transport(crate::native::NativeObservationError),
    /// Complete message schema, direction or bound rejected.
    Wire(RecoveryWireError),
    /// Original Launch bytes failed their authoritative Entry grammar.
    Shape(crate::coordinator_entry::EntryTransportError),
    /// Actual bounded storage allocation failed.
    Allocation(std::collections::TryReserveError),
    /// Source/session/grant association does not match.
    Binding,
    /// Bytes, total, acknowledgement or sequence arrived out of order or repeated.
    Order,
}
impl std::fmt::Display for RecoveryStartupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let category = match self {
            Self::Admission(_) => "admission",
            Self::Transport(_) => "transport",
            Self::Wire(_) => "wire",
            Self::Shape(_) => "shape",
            Self::Allocation(_) => "allocation",
            Self::Binding => "binding",
            Self::Order => "order",
        };
        write!(f, "Authenticated recovery startup rejected: {category}")
    }
}
impl std::error::Error for RecoveryStartupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(e) => Some(e),
            Self::Transport(e) => Some(e),
            Self::Wire(e) => Some(e),
            Self::Shape(e) => Some(e),
            Self::Allocation(e) => Some(e),
            _ => None,
        }
    }
}
/// Own the fixed preclaimed receiver before any Root Launch body is adopted.
/// At most one body and one immutable acknowledgement belong to the original session.
/// Cancellation/failed shape retains this owner and its original credit for cleanup.
pub struct RecoveryStartupAssembly {
    inner: Box<Assembly>,
    charge: storage::Charge,
}
struct Assembly {
    source: Source,
    total: Option<usize>,
    filled: usize,
    root_total: Option<usize>,
    root_filled: usize,
    sequence: u64,
    acknowledgement: Option<DurableAcknowledgement>,
}
/// Own the genuine same-process recovery origin independently of a startup transfer.
/// Clones retain identical Root authentication, original budget and memory/source.
/// No public facts, tokens, child paths or serialized material can construct it.
#[derive(Clone)]
pub struct AuthenticatedRecoveryOrigin(pub(crate) SourceRef);
impl AuthenticatedRecoveryOrigin {
    /// Return complete matching facts without granting another delegation.
    pub fn key(&self) -> RecoveryGrantKey {
        self.0.key
    }
    /// Borrow the original authenticated Launch grammar without constructing another grant.
    /// # Errors
    /// Preserves current original source rejection and authoritative Launch decoding failure.
    pub fn view(
        &self,
    ) -> Result<
        sugarsubstitute_contracts::coordinator_entry::EntryLaunchRequestView<'_>,
        RecoveryStartupError,
    > {
        self.check().map_err(RecoveryStartupError::Admission)?;
        crate::coordinator_entry::startup_launch_payload::decode(&self.0.body[..self.0.length])
            .map_err(RecoveryStartupError::Shape)
    }
    /// Borrow the exact Main registration identity received from the original Root Record.
    /// # Errors
    /// Preserves source rejection and rejects an incomplete authenticated startup.
    pub fn desktop_identity(
        &self,
    ) -> Result<sugarsubstitute_contracts::startup::MainIdentity, RecoveryStartupError> {
        self.check().map_err(RecoveryStartupError::Admission)?;
        self.0.desktop_identity.ok_or(RecoveryStartupError::Binding)
    }
    /// Return the immutable original cross-process cutoff, without clock translation.
    pub fn original_counter(&self) -> CounterDeadline {
        self.0.counter
    }
    /// Return its one actual conservative translation in this execution owner's epoch.
    pub fn original_until(&self) -> Moment {
        self.0.until
    }
    /// Borrow the authenticated Root's configured capture-root UTF16 units.
    /// Actual protected-path/account/reparse and root-file identity verification is
    /// mandatory before creation. These path bytes alone grant no filesystem privilege.
    pub fn capture_root_units(&self) -> impl Iterator<Item = u16> + '_ {
        self.0.root_units_le[..self.0.root_length]
            .chunks_exact(2)
            .map(|units| u16::from_le_bytes([units[0], units[1]]))
    }
    /// Return matching actual Root directory identity to compare after fresh protected open.
    pub fn capture_root_identity(&self) -> [u8; 24] {
        self.0.root_identity
    }
    /// Bind a fresh capture job to this source's exact executing Core and original cutoff.
    /// Native capture-bundle admission independently authenticates the actual running job.
    /// # Errors
    /// Rejects foreign execution, purpose/class, extended cutoff or closed original source.
    pub fn check_capture_context(&self, context: &JobContext) -> Result<(), AdmissionError> {
        self.check()?;
        if context.owner_id() != self.0.execution_id()
            || context.spec().operation != Operation::NativeLifecycle
            || context.spec().class != JobClass::InfrastructureLifecycle
            || context.spec().reservation_bytes != 4 * 1024 * 1024
            || context.deadline().is_none_or(|until| until > self.0.until || context.now() >= until)
        {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        Ok(())
    }
    /// Match an already-owned input's close-only context after Root source admission closes.
    /// This permits no creation, capture, resume, termination or new inherited capability.
    /// # Errors
    /// Rejects foreign/fabricated context, wrong purpose/class, missing finite cutoff,
    /// actual stopped worker, changed memory association or expired cleanup scope.
    pub fn check_retirement_context(&self, context: &JobContext) -> Result<(), AdmissionError> {
        self.0.check_retirement_context(context)
    }
    /// Check the actual original Root session, successful issuer and every current scope.
    /// # Errors
    /// Preserves actual provider/source/cancellation/expiry rejection.
    pub fn check(&self) -> Result<(), AdmissionError> {
        self.0.check()
    }
}
/// Return both genuine origins: fresh capture owns one and captured Desktop startup the other.
/// These same-source aliases do not duplicate a native registration or startup grant.
pub struct RecoveryStartup {
    /// Actual authenticated Root grant for fresh-process capture registration.
    pub origin: AuthenticatedRecoveryOrigin,
    /// One original Launch body for the fresh supervisor's captured Desktop section.
    pub startup: PreparedStartupSnapshot,
}
impl crate::ExecutionClient {
    /// Admit fixed receiver construction through the central finite projection lane.
    /// Only an authenticated child-side recovery session may supply the original source.
    /// The exact four-MiB projection contains a fixed135168-byte receiver claim; native
    /// pipe buffers stay charged to their separate persistent session. No worker waits
    /// for input: the returned owner consumes later real frame guards on application turns.
    /// # Errors
    /// Rejects foreign/stopped sessions, wrong original deadline/class/reservation,
    /// current scope/cancellation, duplicate receiver or actual queue/memory saturation.
    pub fn submit_recovery_startup_assembly(
        &self,
        scope: &Scope,
        spec: JobSpec,
        control: &RecoveryPipeControl,
    ) -> Result<OwnedJob<RecoveryStartupAssembly, RecoveryStartupError>, AdmissionError> {
        if spec.operation != Operation::NativeLifecycle
            || spec.class != JobClass::InfrastructureLifecycle
            || spec.reservation_bytes != 4 * 1024 * 1024
            || spec.deadline != Some(control.original_deadline())
            || spec.replacement.is_some()
            || !control.inner.shared.child
            || !control.inner.shared.core.ptr_eq(&Arc::downgrade(&self.core))
        {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        let shared = control.inner.shared.clone();
        let owner = control.inner.owner.clone();
        let core = self.core.clone();
        self.submit_blocking(scope, spec, move |context| {
            Source::allocate(core, shared, owner, context).map(|(source, charge)| {
                RecoveryStartupAssembly {
                    charge,
                    inner: Box::new(Assembly {
                        source,
                        total: None,
                        filled: 0,
                        root_total: None,
                        root_filled: 0,
                        sequence: 0,
                        acknowledgement: None,
                    }),
                }
            })
        })
    }
}
impl RecoveryStartupAssembly {
    /// Copy one real same-session frame into its exact original destination.
    /// Validation precedes mutations; rejection leaves original frame custody borrowed
    /// with the caller and never expands the receiver or renews its cutoff.
    /// # Errors
    /// Rejects foreign frame guards, replay/gaps, changed grants/totals, noncontiguous
    /// ranges, premature/repeated ACK, malformed shape, cancellation or original expiry.
    pub fn receive(&mut self, frame: &ReceivedLifecycleFrame) -> Result<(), RecoveryStartupError> {
        self.inner.source.check().map_err(RecoveryStartupError::Admission)?;
        if !frame.shared.same_allocation(&self.inner.source.shared) {
            return Err(RecoveryStartupError::Binding);
        }
        let decoded = decode(frame.as_bytes(), RecoveryReceiver::Supervisor)
            .map_err(RecoveryStartupError::Wire)?;
        if decoded.key != self.inner.source.key
            || self.inner.sequence.checked_add(1) != Some(decoded.sequence)
        {
            return Err(RecoveryStartupError::Order);
        }
        match decoded.command {
            RecoveryCommand::Startup { total, offset, bytes } => {
                let total = total as usize;
                if self.inner.acknowledgement.is_some()
                    || self.inner.total.is_some_and(|old| old != total)
                    || offset as usize != self.inner.filled
                    || total > self.inner.source.body.len()
                    || self.inner.filled.checked_add(bytes.len()).is_none_or(|end| end > total)
                {
                    return Err(RecoveryStartupError::Order);
                }
                let end = self.inner.filled + bytes.len();
                self.inner.source.body[self.inner.filled..end].copy_from_slice(bytes);
                self.inner.filled = end;
                self.inner.total = Some(total);
            }
            RecoveryCommand::CaptureRoot { identity, total, offset, units_le } => {
                let total = total as usize;
                if self.inner.acknowledgement.is_some()
                    || self.inner.root_total.is_some_and(|old| old != total)
                    || (self.inner.root_filled != 0 && self.inner.source.root_identity != identity)
                    || offset as usize != self.inner.root_filled
                    || total > self.inner.source.root_units_le.len()
                    || self
                        .inner
                        .root_filled
                        .checked_add(units_le.len())
                        .is_none_or(|end| end > total)
                {
                    return Err(RecoveryStartupError::Order);
                }
                let end = self.inner.root_filled + units_le.len();
                self.inner.source.root_units_le[self.inner.root_filled..end]
                    .copy_from_slice(units_le);
                self.inner.source.root_identity = identity;
                self.inner.root_filled = end;
                self.inner.root_total = Some(total);
            }
            RecoveryCommand::DesktopIdentity { launch, navigation } => {
                if self.inner.acknowledgement.is_some()
                    || self.inner.source.desktop_identity.is_some()
                {
                    return Err(RecoveryStartupError::Order);
                }
                self.inner.source.desktop_identity =
                    Some(sugarsubstitute_contracts::startup::MainIdentity {
                        run: self.inner.source.key.run(),
                        launch,
                        navigation,
                    });
            }
            RecoveryCommand::Commit(acknowledgement) => {
                if self.inner.acknowledgement.is_some()
                    || self.inner.source.desktop_identity.is_none()
                    || self.inner.total != Some(self.inner.filled)
                    || self.inner.filled == 0
                    || self.inner.root_total != Some(self.inner.root_filled)
                    || self.inner.root_filled == 0
                {
                    return Err(RecoveryStartupError::Order);
                }
                startup_launch_payload::decode(&self.inner.source.body[..self.inner.filled])
                    .map_err(RecoveryStartupError::Shape)?;
                self.inner.acknowledgement = Some(acknowledgement);
            }
            _ => return Err(RecoveryStartupError::Order),
        }
        self.inner.sequence = decoded.sequence;
        Ok(())
    }
    /// Publish the same completed authenticated body, once, without another allocation claim.
    /// The actor preserves the received sequence for later recovery-control frames, so a
    /// retransmitted startup or ACK cannot become another native grant.
    /// # Errors
    /// Returns the unchanged receiver with its cause if ACK/body/current source is incomplete.
    pub fn finish(self) -> Result<(RecoveryStartup, u64), (RecoveryStartupError, Self)> {
        let decision =
            self.inner.source.check().map_err(RecoveryStartupError::Admission).and_then(|()| {
                if self.inner.acknowledgement.is_none()
                    || self.inner.source.desktop_identity.is_none()
                    || self.inner.total != Some(self.inner.filled)
                    || self.inner.filled == 0
                    || self.inner.root_total != Some(self.inner.root_filled)
                    || self.inner.root_filled == 0
                {
                    Err(RecoveryStartupError::Order)
                } else {
                    Ok(())
                }
            });
        if let Err(cause) = decision {
            return Err((cause, self));
        }
        let sequence = self.inner.sequence;
        self.inner.source.enrollment_sequence.store(sequence, std::sync::atomic::Ordering::Release);
        let Self { inner, charge } = self;
        let Assembly { mut source, filled, root_filled, .. } = *inner;
        source.length = filled;
        source.root_length = root_filled;
        let source = SourceRef::publish(source, charge);
        let startup = PreparedStartupSnapshot::delegated(source.clone());
        Ok((RecoveryStartup { origin: AuthenticatedRecoveryOrigin(source), startup }, sequence))
    }
}
