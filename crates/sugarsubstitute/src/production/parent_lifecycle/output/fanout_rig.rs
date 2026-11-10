// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real execution and Parent request storage; only external output reads/close are substituted.
use super::*;
use crate::completion_wake::Wake;
use std::sync::Arc;
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};
#[derive(Default)]
struct Clock;
impl sugarsubstitute_contracts::execution::Clock for Clock {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
pub(super) struct Rig {
    pub(super) client: ExecutionClient,
    pub(super) native: Scope,
    wake: Arc<Wake>,
    execution: ExecutionOwner,
}
impl Rig {
    pub(super) fn new() -> TestResult<Self> {
        let wake = Arc::new(Wake::default());
        let execution = ExecutionOwner::new(Arc::new(Clock), wake.clone())?;
        let client = execution.client();
        let native =
            client.create_native_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        Ok(Self { client, native, wake, execution })
    }
    pub(super) fn until(&self, mut ready: impl FnMut() -> TestResult<bool>) -> TestResult<()> {
        self.wake.until(|| {
            if let Err(cause) = self.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))
            {
                return Some(Err(cause.into()));
            }
            match ready() {
                Ok(true) => Some(Ok(())),
                Ok(false) => None,
                Err(cause) => Some(Err(cause)),
            }
        })
    }
    pub(super) fn joined(self) {
        drop(self.execution);
        assert_eq!(self.native.snapshot().reserved_bytes, 0);
    }
}
/// Match production heap placement under the same original actor/reply/cause reservation.
/// Keep the single construction return temporary out of each multi-Parent test frame.
#[inline(never)]
pub(super) fn parent(lease: LeaseKey) -> TestResult<Box<ParentLifecycle>> {
    let storage = crate::production::coordinator_backend::tests::storage_tests::claim(
        ParentLifecycle::storage_bytes(),
    )
    .map_err(|cause| cause as Box<dyn Error>)?;
    let funded = ParentLifecycle::fixed_storage_bytes()
        .checked_add(std::mem::size_of::<Box<ParentLifecycle>>() as u64)
        .ok_or("Parent storage arithmetic overflow")?;
    if funded > storage.bytes() {
        return Err("Parent fixture allocation exceeds its original reservation".into());
    }
    let mut causes =
        Some(crate::production::parent_lifecycle::causes::BackendCauses::new(storage)?);
    let mut parent = Box::new(ParentLifecycle::new(
        lease.run,
        sugarsubstitute_contracts::startup::LaunchId::new(1)?,
        sugarsubstitute_contracts::configuration::ProfileId::new(format!(
            "Fanout_{}",
            lease.run.instance.get()
        ))?,
        lease.connection,
        sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeDeadline::new(
            10_000_000_000,
            1_000_000_000,
        )?,
        &mut causes,
    )?);
    parent.begin_output_sequence(lease)?;
    Ok(parent)
}
pub(super) fn pull(
    parent: &mut ParentLifecycle,
    lease: LeaseKey,
    backend: &mut crate::production::coordinator_backend::CoordinatorBackend,
    transcript: &mut TranscriptStore,
) -> TestResult<OutputFacts> {
    use sugarsubstitute_supervision::lifecycle_protocol::{
        FrameCodec, WireHeader, WireMessage, WirePayload,
    };
    let next = parent.next_delivery.checked_add(1).ok_or("Delivery exhausted")?;
    let id = NonZeroU64::new(next).ok_or("Delivery absent")?;
    let header = WireHeader {
        run: lease.run,
        connection: Some(lease.connection),
        request: Some(next_request(parent)?),
        deadline_qpc: 10_000_000_000,
    };
    let mut bytes = [0; 512];
    let count = FrameCodec::encode(
        &WireMessage {
            header,
            payload: WirePayload::OutputPull {
                lease: crate::production::parent_lifecycle::replies::reference(lease),
                acknowledged_delivery: parent.acknowledged_output.map(|(_, id)| id),
            },
        },
        &mut bytes,
    )?;
    let (index, fresh) = parent.requests.admit(header, &bytes[..count])?;
    assert!(fresh);
    parent
        .retain_output_delivery(index, lease, id, backend)
        .map_err(|cause| cause as Box<dyn Error>)?;
    let record = parent.requests.get(index)?;
    let Some(Reply::Output(facts)) = record.reply.as_ref() else {
        return Err("Expected real captured output, not idle".into());
    };
    let batch = OutputBatch::new(
        lease.resource,
        facts.stream,
        facts.sequence,
        &record.output_bytes[..facts.count],
        facts.gap,
        facts.end,
    )?;
    transcript.apply(&batch)?;
    Ok(*facts)
}
pub(super) fn text(store: &TranscriptStore) -> TestResult<Vec<String>> {
    Ok(store.snapshot_tail(16)?.records().map(|record| record.text().to_owned()).collect())
}

fn next_request(
    parent: &ParentLifecycle,
) -> TestResult<sugarsubstitute_contracts::lifecycle::RequestId> {
    let latest = (0..crate::production::parent_lifecycle::history::SLOTS)
        .filter_map(|index| parent.requests.get(index).ok())
        .filter_map(|record| record.header.request)
        .map(|id| id.get())
        .max()
        .unwrap_or(0);
    Ok(sugarsubstitute_contracts::lifecycle::RequestId::new(
        latest.checked_add(1).ok_or("Request exhausted")?,
    )?)
}
/// Exercise the same already-funded Parent delivery path, retaining an actual Idle reply.
pub(super) fn idle(
    parent: &mut ParentLifecycle,
    lease: LeaseKey,
    backend: &mut crate::production::coordinator_backend::CoordinatorBackend,
) -> TestResult<()> {
    use sugarsubstitute_supervision::lifecycle_protocol::{
        FrameCodec, WireHeader, WireMessage, WirePayload,
    };
    let header = WireHeader {
        run: lease.run,
        connection: Some(lease.connection),
        request: Some(next_request(parent)?),
        deadline_qpc: 10_000_000_000,
    };
    let mut bytes = [0; 512];
    let count = FrameCodec::encode(
        &WireMessage {
            header,
            payload: WirePayload::OutputPull {
                lease: crate::production::parent_lifecycle::replies::reference(lease),
                acknowledged_delivery: parent.acknowledged_output.map(|(_, id)| id),
            },
        },
        &mut bytes,
    )?;
    let (index, fresh) = parent.requests.admit(header, &bytes[..count])?;
    assert!(fresh);
    let next = NonZeroU64::new(parent.next_delivery.checked_add(1).ok_or("Delivery exhausted")?)
        .ok_or("Delivery absent")?;
    parent
        .retain_output_delivery(index, lease, next, backend)
        .map_err(|cause| cause as Box<dyn Error>)?;
    assert!(
        matches!(parent.requests.get(index)?.reply, Some(Reply::OutputIdle(actual)) if actual == lease)
    );
    assert!(parent.output.is_none());
    assert!(parent.output_receive_until.is_none());
    Ok(())
}
