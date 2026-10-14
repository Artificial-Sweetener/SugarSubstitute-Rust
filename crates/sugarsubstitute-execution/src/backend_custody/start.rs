// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind an already-admitted custody driver only after its durable source is charged.
use super::*;
use crate::scope::lock;
use std::sync::{Arc, Mutex};
use sugarsubstitute_contracts::execution::OwnedJob;

enum Phase {
    Pending,
    Bound(worker::Factory),
    Abandoned,
    Consumed,
}
pub(crate) struct Binding {
    owner: Option<Arc<Mutex<Phase>>>,
    metadata: BackendCustodyMemory,
}
impl Binding {
    pub(crate) fn new(metadata: BackendCustodyMemory) -> Self {
        Self { owner: Some(Arc::new(Mutex::new(Phase::Pending))), metadata }
    }
    pub(crate) fn ready(&self) -> bool {
        self.owner.as_ref().is_some_and(|owner| !matches!(*lock(owner), Phase::Pending))
    }
    pub(crate) fn take(&self) -> Option<worker::Factory> {
        let owner = self.owner.as_ref()?;
        let original = std::mem::replace(&mut *lock(owner), Phase::Consumed);
        match original {
            Phase::Bound(factory) => Some(factory),
            _ => None,
        }
    }
    fn abandon(&self) {
        if let Some(owner) = &self.owner {
            let mut phase = lock(owner);
            if matches!(*phase, Phase::Pending) {
                *phase = Phase::Abandoned;
            }
        }
    }
}
impl Clone for Binding {
    fn clone(&self) -> Self {
        Self { owner: self.owner.clone(), metadata: self.metadata.clone() }
    }
}
impl Drop for Binding {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.take() {
            drop(Arc::into_inner(owner));
        }
    }
}

/// Own the one actual worker reservation while constructing its funded durable source.
/// Binding transfers the inert driver without a second queue/capacity decision.
/// Consume or drop this owner before joining execution; Drop abandons only an
/// unbound factory and keeps its actual cancellation/terminal observation owned.
pub struct BackendCustodyWorkerStart {
    pub(crate) job: Option<OwnedJob<BackendCustodyWorkerReport, BackendCustodyWorkerFailure>>,
    pub(crate) binding: Binding,
}
impl BackendCustodyWorkerStart {
    /// Borrow the original fixed metadata credit before source allocation.
    pub fn metadata(&self) -> &BackendCustodyMemory {
        &self.binding.metadata
    }
    /// Return actual execution-side controls to include in source layout preflight.
    pub const fn control_storage_bytes() -> usize {
        size_of::<Self>()
            + size_of::<worker::Work>()
            + size_of::<worker::Active>()
            + size_of::<Mutex<Phase>>()
            + 2 * size_of::<usize>()
            + BackendCustodyMemory::control_storage_bytes()
    }
    /// Cancel the identical unbound original job and retain its typed terminal observer.
    /// Abandons only the private pending binding, requests actual Core cancellation,
    /// and transfers its original job without a new scope, deadline or reservation.
    /// The caller retains the returned observer and metadata aliases until actual
    /// terminal delivery and physical disposal; cancellation proves no native stop.
    pub fn cancel(
        mut self,
    ) -> Option<OwnedJob<BackendCustodyWorkerReport, BackendCustodyWorkerFailure>> {
        self.binding.abandon();
        if let Some(job) = &self.job {
            job.cancel();
        }
        self.binding.metadata.changed();
        self.job.take()
    }
    /// Transfer one inert driver factory into this admitted original reservation.
    /// Source data/control must retain `metadata` outside their last allocation.
    /// Factory construction runs later on the actual native worker outside locks;
    /// its native owner must exist before initialization invokes any effects.
    /// # Errors
    /// Returns this unchanged start and factory if their compiled storage cannot fit.
    pub fn bind<M>(
        mut self,
        factory: M,
    ) -> Result<OwnedJob<BackendCustodyWorkerReport, BackendCustodyWorkerFailure>, (Self, M)>
    where
        M: FnOnce(
                &sugarsubstitute_contracts::execution::JobContext,
                &BackendCustodyMemory,
            )
                -> Result<Box<dyn BackendCustodyDriver>, crate::native::NativeObservationError>
            + Send
            + 'static,
    {
        if size_of::<M>() + Self::control_storage_bytes() > self.metadata().bytes() as usize
            || self.job.is_none()
            || self.binding.owner.is_none()
        {
            return Err((self, factory));
        }
        let Some(job) = self.job.take() else {
            return Err((self, factory));
        };
        let Some(owner) = &self.binding.owner else {
            self.job = Some(job);
            return Err((self, factory));
        };
        let mut phase = lock(owner);
        if !matches!(*phase, Phase::Pending) {
            drop(phase);
            self.job = Some(job);
            return Err((self, factory));
        }
        *phase = Phase::Bound(Box::new(factory));
        drop(phase);
        self.binding.metadata.changed();
        Ok(job)
    }
}
impl Drop for BackendCustodyWorkerStart {
    fn drop(&mut self) {
        if let Some(job) = &self.job {
            self.binding.abandon();
            job.cancel();
            self.binding.metadata.changed();
        }
    }
}
