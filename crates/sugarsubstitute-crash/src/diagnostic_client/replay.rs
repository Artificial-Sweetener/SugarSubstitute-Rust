// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain eight exact results and one issued command beyond result eviction.
use super::*;
use sha2::{Digest, Sha256};
use sugarsubstitute_contracts::{incident::*, lifecycle::CoordinatorKey};
/// Literal saved result inventory, independent of request lifetime count.
pub const DIAGNOSTIC_RESULT_SLOTS: usize = 8;
/// Saved bounded response facts; construction alone proves no durable/native effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticReply {
    /// Exact original categorical rejection/failure.
    Rejected(DiagnosticWireError),
    /// Actual categorical operation failure; original causes remain adapter-owned.
    Failed(DiagnosticOperationFailure),
    /// Exact original accepted bounded byte count and complete digest.
    Completed {
        /// Actually reported bytes.
        bytes: u64,
        /// Actually reported digest.
        sha256: [u8; 32],
        /// Reported committed revision, if applicable.
        revision: Option<IncidentRevision>,
    },
}
#[derive(Clone, Copy)]
struct Entry {
    request: DiagnosticRequestId,
    digest: [u8; 32],
    reply: DiagnosticReply,
}
struct Pending {
    context: DiagnosticContext,
    kind: DiagnosticRequestKind,
    request: DiagnosticRequestId,
    digest: [u8; 32],
    generation: u64,
}
/// One nonclone issued correlation; Drop cannot abandon the router's pending work.
pub struct IssuedDiagnosticRequest {
    key: DiagnosticKey,
    request: DiagnosticRequestId,
    digest: [u8; 32],
    generation: u64,
}
impl IssuedDiagnosticRequest {
    /// Exact original request identity, without native authority.
    pub const fn request(&self) -> DiagnosticRequestId {
        self.request
    }
}
/// Decide replay/admission without reexecuting retained or evicted commands.
pub enum DiagnosticAdmission {
    /// First request, retained until exact terminal observation.
    Execute(IssuedDiagnosticRequest),
    /// Exact duplicate is still pending; no second effect admitted.
    Pending,
    /// Exact saved original result; no second effect admitted.
    Replay(DiagnosticReply),
}
/// One exact helper-session replay owner; frame bytes remain with transport.
pub struct DiagnosticRequestRouter {
    key: DiagnosticKey,
    high_water: u64,
    generation: u64,
    cursor: usize,
    history: [Option<Entry>; DIAGNOSTIC_RESULT_SLOTS],
    pending: Option<Pending>,
    operation: Option<DiagnosticContext>,
}
impl DiagnosticRequestRouter {
    /// Allocate no native owner or heap state; key is matching data, not authentication.
    pub const fn new(key: DiagnosticKey) -> Self {
        Self {
            key,
            high_water: 0,
            generation: 0,
            cursor: 0,
            history: [None; DIAGNOSTIC_RESULT_SLOTS],
            pending: None,
            operation: None,
        }
    }
    /// Observe high-water even after all saved results have rotated.
    pub const fn high_water(&self) -> u64 {
        self.high_water
    }
    /// Admit a strictly decoded original command once; rejection preserves all state.
    /// # Errors
    /// Rejects foreign session, changed/retired duplicates, live work or changed original scope.
    pub fn admit(&mut self, bytes: &[u8]) -> Result<DiagnosticAdmission, DiagnosticWireError> {
        let frame = decode(bytes)?;
        if frame.key != self.key {
            return Err(DiagnosticWireError::Identity);
        }
        let kind = DiagnosticRequestKind::of(frame.command)?;
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        if let Some(p) = &self.pending
            && p.request == frame.request
        {
            return if p.digest == digest {
                Ok(DiagnosticAdmission::Pending)
            } else {
                Err(DiagnosticWireError::ChangedDuplicate)
            };
        }
        if let Some(e) = self.history.iter().flatten().find(|e| e.request == frame.request) {
            return if e.digest == digest {
                Ok(DiagnosticAdmission::Replay(e.reply))
            } else {
                Err(DiagnosticWireError::ChangedDuplicate)
            };
        }
        if frame.request.get() <= self.high_water {
            return Err(DiagnosticWireError::RetiredOperation);
        }
        if self.pending.is_some() {
            return Err(DiagnosticWireError::Busy);
        }
        let generation = self.generation.checked_add(1).ok_or(DiagnosticWireError::Capacity)?;
        if let Some(original) = self.operation
            && frame.context.operation.is_some()
            && (original.operation != frame.context.operation
                || original.deadline != frame.context.deadline
                || original.at != frame.context.at)
        {
            return Err(DiagnosticWireError::Time);
        }
        if let Some(original) = self.operation
            && frame.command == DiagnosticCommand::Retire
            && frame.context.deadline != original.deadline
        {
            return Err(DiagnosticWireError::Time);
        }
        let issued =
            IssuedDiagnosticRequest { key: self.key, request: frame.request, digest, generation };
        if self.operation.is_none() && frame.context.operation.is_some() {
            self.operation = Some(frame.context)
        }
        self.pending = Some(Pending {
            context: frame.context,
            kind,
            request: frame.request,
            digest,
            generation,
        });
        self.high_water = frame.request.get();
        self.generation = generation;
        Ok(DiagnosticAdmission::Execute(issued))
    }
    /// Accept only an exact original result envelope; peer authentication stays transport-owned.
    /// # Errors
    /// Foreign kind/session/request/context/digest preserves original pending custody.
    pub fn accept_result(
        &mut self,
        issued: &IssuedDiagnosticRequest,
        frame: DiagnosticFrame<'_>,
    ) -> Result<(), DiagnosticWireError> {
        validation::check(&frame)?;
        let DiagnosticCommand::Result { kind, request_sha256, reply } = frame.command else {
            return Err(DiagnosticWireError::Shape);
        };
        let pending = self.pending.as_ref().ok_or(DiagnosticWireError::Stale)?;
        if frame.key != self.key
            || frame.request != pending.request
            || kind != pending.kind
            || frame.context != pending.context
            || request_sha256 != pending.digest
        {
            return Err(DiagnosticWireError::Stale);
        }
        self.complete(issued, reply)
    }
    /// Save only the exact issued terminal; wrong token leaves the original pending.
    /// # Errors
    /// Rejects stale/foreign generation or changed fingerprint without disposing custody.
    pub fn complete(
        &mut self,
        issued: &IssuedDiagnosticRequest,
        reply: DiagnosticReply,
    ) -> Result<(), DiagnosticWireError> {
        if matches!(reply,DiagnosticReply::Completed{bytes,..} if bytes>268_435_456) {
            return Err(DiagnosticWireError::Capacity);
        }
        let p = self.pending.as_ref().ok_or(DiagnosticWireError::Stale)?;
        if self.key != issued.key
            || p.request != issued.request
            || p.digest != issued.digest
            || p.generation != issued.generation
        {
            return Err(DiagnosticWireError::Stale);
        }
        self.history[self.cursor] = Some(Entry { request: p.request, digest: p.digest, reply });
        self.cursor = (self.cursor + 1) % DIAGNOSTIC_RESULT_SLOTS;
        self.pending = None;
        Ok(())
    }
}
/// Parent-retained new-operation high-water across transient helper sessions.
/// This inert fence grants no reconciliation or successor launch authority.
pub struct DiagnosticOperationFence {
    owner: CoordinatorKey,
    high_water: u64,
}
impl DiagnosticOperationFence {
    /// Keep the exact current owner without rebasing historical journals.
    pub const fn new(owner: CoordinatorKey) -> Self {
        Self { owner, high_water: 0 }
    }
    /// Admit only a fresh operation; actual predecessor settlement is independently required.
    /// # Errors
    /// Foreign owners or reused/evicted positive IDs reject without changing high-water.
    pub fn admit_new(&mut self, key: LogicalStoreOperationKey) -> Result<(), DiagnosticWireError> {
        if key.owner() != self.owner {
            return Err(DiagnosticWireError::Identity);
        }
        if key.id().get() <= self.high_water {
            return Err(DiagnosticWireError::RetiredOperation);
        }
        self.high_water = key.id().get();
        Ok(())
    }
    /// Observe the retained maximum.
    pub const fn high_water(&self) -> u64 {
        self.high_water
    }
}
