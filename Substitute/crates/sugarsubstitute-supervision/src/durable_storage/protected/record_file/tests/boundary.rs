// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain externally observed fake output slots before primitive failures.
mod exclusion;
mod record;
use super::*;
use crate::{
    coordinator_entry::fixture::{ClockPort, Gate},
    durable_storage::protected::exclusion::fixture::ExclusionFixtureBoundary,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, atomic::Ordering},
};
use sugarsubstitute_contracts::execution::{ExecutionId, JobContext, MemoryClaim, Moment, ScopeId};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Effect {
    Create,
    Write,
    Activate,
}
#[derive(Default)]
pub(super) struct Faults {
    pub(super) adopt: bool,
    pub(super) create: bool,
    pub(super) write: bool,
    pub(super) activate: bool,
    pub(super) flush: bool,
    pub(super) close: bool,
    pub(super) read: bool,
    pub(super) changed_path: bool,
}
struct Exclusion {
    inode: [u8; 24],
    path: PathBuf,
    sid: Vec<u8>,
    memory: Arc<MemoryClaim>,
    execution: ExecutionId,
    cleanup: Option<(ScopeId, Moment)>,
    returned: bool,
}
struct ExternalStage {
    identity: [u8; 24],
    path: PathBuf,
    bytes: Vec<u8>,
    live: bool,
    memory: Option<Arc<MemoryClaim>>,
}
pub(super) struct Disk {
    exclusion: Option<Exclusion>,
    closed: Option<[u8; 24]>,
    stage: Option<ExternalStage>,
    target: Option<Vec<u8>>,
    target_identity: [u8; 24],
    next_stage: u8,
    close_after_activation: bool,
    reads: Vec<Arc<MemoryClaim>>,
    pub(super) faults: Faults,
    pub(super) effects: Vec<Effect>,
    pub(super) gate: Option<(Effect, Arc<Gate>)>,
    pub(super) expiry: Option<(Effect, Arc<ClockPort>, Moment)>,
}
#[derive(Clone)]
pub(crate) struct Fake(pub(super) Arc<Mutex<Disk>>);
impl Fake {
    pub(super) fn new(origin: RecordRead<128>) -> Self {
        let target = match origin {
            RecordRead::Missing => None,
            RecordRead::Empty => Some(Vec::new()),
            RecordRead::Present(bytes) => Some(bytes.to_vec()),
        };
        Self(Arc::new(Mutex::new(Disk {
            exclusion: None,
            closed: None,
            stage: None,
            target,
            target_identity: [10; 24],
            next_stage: 12,
            close_after_activation: false,
            reads: Vec::new(),
            faults: Faults::default(),
            effects: Vec::new(),
            gate: None,
            expiry: None,
        })))
    }
    pub(crate) fn with_bytes(bytes: Option<&[u8]>) -> Self {
        let fake = Self::new(RecordRead::Missing);
        lock(&fake.0).target = bytes.map(<[u8]>::to_vec);
        fake
    }
    pub(crate) fn fail_flush(&self, value: bool) {
        lock(&self.0).faults.flush = value;
    }
    pub(crate) fn fail_write(&self, value: bool) {
        lock(&self.0).faults.write = value;
    }
    pub(crate) fn fail_close(&self, value: bool) {
        lock(&self.0).faults.close = value;
    }
    pub(crate) fn fail_activated_close(&self, value: bool) {
        lock(&self.0).close_after_activation = value;
    }
    pub(crate) fn effect_count(&self) -> usize {
        lock(&self.0).effects.len()
    }
    pub(crate) fn replace_target_identity(&self, identity: [u8; 24]) {
        lock(&self.0).target_identity = identity;
    }
    pub(crate) fn target_identity(&self) -> [u8; 24] {
        lock(&self.0).target_identity
    }
    /// Observe physical fixture closure without acknowledging the record owner.
    pub(super) fn externally_retire(&self, context: &JobContext) -> Result<(), ClaimError> {
        let mut disk = lock(&self.0);
        let exclusion = disk
            .exclusion
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("external_exclusion_missing"))?;
        if exclusion.execution != context.owner_id() {
            return Err(ClaimError::invalid("foreign_external_fixture_retirement"));
        }
        disk.closed = Some(exclusion.inode);
        if let Some(stage) = disk.stage.as_mut() {
            stage.live = false;
            stage.memory = None;
        }
        disk.reads.clear();
        disk.exclusion = None;
        Ok(())
    }
    pub(crate) fn live(&self) -> usize {
        let disk = lock(&self.0);
        usize::from(disk.exclusion.is_some())
            + usize::from(disk.stage.as_ref().is_some_and(|stage| stage.live))
            + disk.reads.len()
    }
    pub(super) fn after(&self, effect: Effect) -> Result<(), ClaimError> {
        let (gate, expiry) = {
            let mut disk = lock(&self.0);
            disk.effects.push(effect);
            if effect == Effect::Activate && disk.close_after_activation {
                disk.faults.close = true;
            }
            (
                disk.gate
                    .as_ref()
                    .filter(|(expected, _)| *expected == effect)
                    .map(|(_, gate)| gate.clone()),
                disk.expiry
                    .as_ref()
                    .filter(|(expected, _, _)| *expected == effect)
                    .map(|(_, clock, until)| (clock.clone(), *until)),
            )
        };
        if let Some((clock, until)) = expiry {
            clock.0.store(until.as_nanos(), Ordering::Release);
        }
        if let Some(gate) = gate {
            gate.enter()?;
        }
        Ok(())
    }
}
