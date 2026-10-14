// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Supply causal boundary controls, deterministic time and actual execution notifications.
use super::*;
#[derive(Default)]
pub(crate) struct ClockPort(pub(crate) AtomicU64);
impl Clock for ClockPort {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::Acquire))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
pub(crate) struct Wake {
    pub(super) revision: Mutex<u64>,
    pub(super) changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *lock(&self.revision) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stage {
    Discover,
    Elect,
    Close,
}
#[derive(Default)]
pub(crate) struct Gate {
    state: Mutex<(bool, bool)>,
    pub(super) changed: Condvar,
}
impl Gate {
    pub(crate) fn enter(&self) -> Result<(), ClaimError> {
        let mut state = lock(&self.state);
        state.0 = true;
        self.changed.notify_all();
        let (_guard, timeout) = self
            .changed
            .wait_timeout_while(state, Duration::from_secs(10), |state| !state.1)
            .map_err(|_| ClaimError::invalid("entry_test_gate_poison"))?;
        if timeout.timed_out() { Err(ClaimError::invalid("entry_test_gate_guard")) } else { Ok(()) }
    }
    pub(crate) fn wait(&self) -> TestResult {
        let (_guard, timeout) = self
            .changed
            .wait_timeout_while(lock(&self.state), Duration::from_secs(10), |state| !state.0)
            .map_err(|_| "entry test gate poisoned")?;
        if timeout.timed_out() { Err("entry test boundary not reached".into()) } else { Ok(()) }
    }
    pub(crate) fn release(&self) {
        lock(&self.state).1 = true;
        self.changed.notify_all();
    }
}
impl Drop for Gate {
    fn drop(&mut self) {
        self.release();
    }
}
pub(crate) struct Fake {
    pub(crate) clock: Arc<ClockPort>,
    pub(crate) gate: Mutex<Option<(Stage, Arc<Gate>)>>,
    pub(crate) failure: Mutex<Option<Stage>>,
    pub(crate) expire_after: Mutex<Option<Stage>>,
    pub(crate) locked: AtomicBool,
    pub(crate) calls: [AtomicUsize; 3],
    pub(crate) close_deadlines: Mutex<Vec<Moment>>,
    pub(crate) origin: Mutex<ElectionRecordOrigin>,
    pub(crate) coordinator_inode: Mutex<Option<[u8; 24]>>,
}
impl Fake {
    pub(crate) fn observe_coordinator(&self, inode: [u8; 24]) {
        *lock(&self.coordinator_inode) = Some(inode);
    }
    pub(crate) fn close_coordinator(&self) {
        *lock(&self.coordinator_inode) = None;
    }

    fn step(
        &self,
        stage: Stage,
        context: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<(), ClaimError> {
        self.calls[match stage {
            Stage::Discover => 0,
            Stage::Elect => 1,
            Stage::Close => 2,
        }]
        .fetch_add(1, Ordering::AcqRel);
        let gate = lock(&self.gate)
            .as_ref()
            .filter(|(expected, _)| *expected == stage)
            .map(|(_, gate)| gate.clone());
        if let Some(gate) = gate {
            gate.enter()?;
        }
        if lock(&self.expire_after).as_ref() == Some(&stage) {
            self.clock.0.store(
                context
                    .deadline()
                    .ok_or_else(|| ClaimError::invalid("entry_test_deadline"))?
                    .as_nanos()
                    + 1,
                Ordering::Release,
            );
        }
        let mut failure = lock(&self.failure);
        if *failure == Some(stage) {
            *failure = None;
            return Err(ClaimError::new(
                "entry_test_native_failure",
                std::io::Error::other("original boundary cause"),
            ));
        }
        Ok(())
    }
}
impl NativePort for Fake {
    fn validate_coordinator(
        &self,
        _: &NativeState,
        _: &crate::native_claims::files::Handle,
        inode: [u8; 24],
        context: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<(), ClaimError> {
        native::check(context)?;
        if *lock(&self.coordinator_inode) == Some(inode) {
            Ok(())
        } else {
            Err(ClaimError::invalid("entry_test_changed_coordinator"))
        }
    }

    fn discover(
        &self,
        state: &mut NativeState,
        _: &std::path::Path,
        _: &std::path::Path,
        context: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<Discovered, ClaimError> {
        if state.memory.is_none() {
            return Err(ClaimError::invalid("entry_test_missing_preclaim"));
        }
        state.namespace = Some(resolved());
        state.image_identity = Some([7; 24]);
        self.step(Stage::Discover, context)?;
        Ok(Discovered { facts: facts()?, installation_identity: [6; 24] })
    }
    fn elect(
        &self,
        state: &mut NativeState,
        context: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<Elected, ClaimError> {
        let origin = *lock(&self.origin);
        state.entry_inode = Some([9; 24]);
        state.record_origin = Some(origin);
        self.locked.store(true, Ordering::Release);
        self.step(Stage::Elect, context)?;
        Ok(match origin {
            ElectionRecordOrigin::Fresh(origin) => Elected::Fresh { origin, inode: [9; 24] },
            ElectionRecordOrigin::Released(record) => {
                Elected::OrdinaryReleased { record, inode: [9; 24] }
            }
            ElectionRecordOrigin::RecoveredHeld { record, coordinator_inode } => {
                Elected::RecoveredHeld { record, inode: [9; 24], coordinator_inode }
            }
        })
    }
    fn close(
        &self,
        state: &mut NativeState,
        context: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<(), ClaimError> {
        let until =
            context.deadline().ok_or_else(|| ClaimError::invalid("entry_test_close_deadline"))?;
        lock(&self.close_deadlines).push(until);
        self.step(Stage::Close, context)?;
        self.locked.store(false, Ordering::Release);
        state.memory = None;
        state.namespace = None;
        state.image_identity = None;
        state.entry_inode = None;
        state.record_origin = None;
        state.retired = true;
        Ok(())
    }
    fn validate_election(
        &self,
        state: &NativeState,
        proof: &RetainedElectionProof,
        _: &sugarsubstitute_contracts::execution::JobContext,
    ) -> Result<(), ClaimError> {
        if self.locked.load(Ordering::Acquire) && state.entry_inode == Some(proof.entry_inode) {
            Ok(())
        } else {
            Err(ClaimError::invalid("entry_test_changed_exclusion"))
        }
    }
}
pub(crate) fn facts() -> Result<EntryNativeIdentityFacts, ClaimError> {
    EntryNativeIdentityFacts::new(
        NamespaceId::new([1; 32]),
        &[1, 1, 0, 0, 0, 0, 0, 5, 42, 0, 0, 0],
        4,
        [7; 24],
    )
    .map_err(|cause| ClaimError::new("entry_test_facts", cause))
}
pub(crate) fn resolved() -> ResolvedNamespace {
    ResolvedNamespace {
        namespace: NamespaceId::new([1; 32]),
        account_sid: vec![1, 1, 0, 0, 0, 0, 0, 5, 42, 0, 0, 0],
        session_id: 4,
        installation_identity: [6; 24],
        directory: PathBuf::from(r"E:\entry-external-boundary"),
        directory_slot: 0,
        directory_identity: [1; 24],
    }
}

impl crate::registry::construction_fixture::ConstructionWake for Wake {
    fn revision(&self) -> u64 {
        *crate::registry::lock(&self.revision)
    }
    fn wait_revision(&self, previous: u64) -> Result<(), Box<dyn std::error::Error>> {
        let guard = crate::registry::lock(&self.revision);
        let (_guard, timed) = self
            .changed
            .wait_timeout_while(guard, std::time::Duration::from_secs(10), |value| {
                *value == previous
            })
            .map_err(|_| "Constructor completion wake poisoned")?;
        if timed.timed_out() {
            Err("Actual constructor completion did not arrive".into())
        } else {
            Ok(())
        }
    }
}
