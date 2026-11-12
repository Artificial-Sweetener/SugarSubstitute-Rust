// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Substitute only external pipe reads/close; central output reservation and worker stay real.
use super::*;
use crate::ExecutionClient;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

struct External {
    data: Mutex<[VecDeque<Vec<u8>>; 2]>,
    eof: [AtomicBool; 2],
    signals: Mutex<Option<[Arc<Signal>; 2]>>,
    closes: AtomicU64,
}
/// Bounded fake external pipe source. This grants no process, OS handle or trusted batch.
/// Available only to boundary-fixture consumers; the actual Core must admit every session.
#[derive(Clone)]
pub struct OutputFixture(Arc<External>);
impl Default for OutputFixture {
    fn default() -> Self {
        Self::new()
    }
}
impl OutputFixture {
    /// Construct inert external state before actual execution admission.
    pub fn new() -> Self {
        Self(Arc::new(External {
            data: Mutex::new([VecDeque::new(), VecDeque::new()]),
            eof: [AtomicBool::new(false), AtomicBool::new(false)],
            signals: Mutex::new(None),
            closes: AtomicU64::new(0),
        }))
    }
    /// Supply one external read of at most32KiB; each stream retains at most128 source chunks.
    /// # Errors
    /// Rejects overflow, empty data or data after actual external EOF without mutation.
    pub fn push(&self, stream: OutputStream, bytes: &[u8]) -> Result<(), NativeObservationError> {
        if bytes.is_empty()
            || bytes.len() > MAX_BATCH_BYTES
            || self.0.eof[stream.index()].load(Ordering::Acquire)
        {
            return Err(invariant("fixture_output_read_shape"));
        }
        {
            let mut data = lock(&self.0.data);
            if data[stream.index()].len() >= 128 {
                return Err(invariant("fixture_output_source_full"));
            }
            data[stream.index()].push_back(bytes.to_vec());
        }
        self.wake(stream);
        Ok(())
    }
    /// Report external EOF after already supplied bytes, preserving normal worker ordering.
    pub fn end(&self, stream: OutputStream) {
        self.0.eof[stream.index()].store(true, Ordering::Release);
        self.wake(stream);
    }
    /// Return actual external close calls, never task or native retirement authority.
    pub fn closes(&self) -> u64 {
        self.0.closes.load(Ordering::Acquire)
    }
    fn wake(&self, stream: OutputStream) {
        let signal = lock(&self.0.signals).as_ref().map(|signals| signals[stream.index()].clone());
        if let Some(signal) = signal {
            signal.complete();
        }
    }
    /// Admit the same production queue/claims/worker with this one external pipe substitute.
    /// # Errors
    /// Returns the original Core admission rejection. Reusing a prepared external source fails
    /// in its actual worker instead of forging a second prepared lease or output batch.
    pub fn reserve(
        &self,
        client: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        identity: OutputIdentity,
    ) -> Result<WindowsOutputSession, AdmissionError> {
        super::reserve(&client.core, scope, spec, identity, Box::new(ExternalLease(self.0.clone())))
    }
}
struct ExternalLease(Arc<External>);
impl ExternalLease {
    fn effect(
        &self,
        stream: OutputStream,
        bytes: &mut [u8],
    ) -> Result<Progress, NativeObservationError> {
        if let Some(source) = lock(&self.0.data)[stream.index()].pop_front() {
            if source.len() > bytes.len() {
                return Err(invariant("fixture_output_destination"));
            }
            bytes[..source.len()].copy_from_slice(&source);
            return Ok(Progress::Bytes(source.len() as u32));
        }
        Ok(if self.0.eof[stream.index()].load(Ordering::Acquire) {
            Progress::Eof
        } else {
            Progress::Pending
        })
    }
}
impl OutputLease for ExternalLease {
    fn prepare(
        &mut self,
        _: OutputIdentity,
        signals: [Arc<Signal>; 2],
    ) -> Result<[usize; 3], NativeObservationError> {
        let mut original = lock(&self.0.signals);
        if original.is_some() {
            return Err(invariant("fixture_output_already_prepared"));
        }
        *original = Some(signals);
        Ok([11, 12, 13])
    }
    fn close_input(&mut self) -> Result<(), NativeObservationError> {
        Ok(())
    }
    fn release_child_ends(&mut self) -> Result<(), NativeObservationError> {
        Ok(())
    }
    fn read(
        &mut self,
        stream: OutputStream,
        bytes: &mut [u8],
    ) -> Result<Progress, NativeObservationError> {
        self.effect(stream, bytes).map(|progress| match progress {
            Progress::Pending => Progress::IssuedPending,
            other => other,
        })
    }
    fn poll(
        &mut self,
        stream: OutputStream,
        bytes: &mut [u8],
    ) -> Result<Progress, NativeObservationError> {
        self.effect(stream, bytes)
    }
    fn quiesce(&mut self, _: OutputStream) -> Result<(), NativeObservationError> {
        Ok(())
    }
    fn take_completed_read(&mut self, _: OutputStream) -> Option<Progress> {
        None
    }
    fn retire(&mut self, pinned: bool) -> Result<bool, NativeObservationError> {
        if pinned {
            return Ok(false);
        }
        self.0.closes.fetch_add(1, Ordering::AcqRel);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_test_support::{Rig, TestResult};
    use sugarsubstitute_contracts::{execution::*, lifecycle::*};
    #[test]
    fn external_output_fixture_rejection_preserves_source_until_genuine_native_admission()
    -> TestResult {
        let rig = Rig::new()?;
        let external = OutputFixture::new();
        external.push(OutputStream::Stdout, b"original")?;
        external.end(OutputStream::Stdout);
        external.end(OutputStream::Stderr);
        let key = ResourceKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([7; 32]),
                epoch: CoordinatorEpoch::new([8; 16])?,
            },
            id: ResourceId::new(1)?,
            generation: ResourceGeneration::new(1)?,
        };
        let spec = JobSpec {
            id: JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::NativeObservation,
            reservation_bytes: MIN_WINDOWS_OUTPUT_BYTES,
            deadline: None,
            replacement: None,
        };
        assert!(
            external.reserve(&rig.client, &rig.scope, spec, OutputIdentity::Managed(key)).is_err()
        );
        assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
        assert_eq!(external.closes(), 0);
        let native = rig
            .client
            .create_native_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
        let mut session =
            external.reserve(&rig.client, &native, spec, OutputIdentity::Managed(key))?;
        let terminal = rig.completion(&mut session.completion)?;
        assert!(matches!(terminal.outcome, JobOutcome::Success(_)));
        let guard = session.control.take_batch()?.ok_or("Original actual batch absent")?;
        assert_eq!(guard.batch().ok_or("Wrong identity kind")?.bytes(), b"original");
        assert_eq!(external.closes(), 1);
        assert!(native.snapshot().reserved_bytes > 0);
        drop((guard, terminal, session));
        drop(rig.owner);
        assert_eq!(native.snapshot().reserved_bytes, 0);
        Ok(())
    }
}
