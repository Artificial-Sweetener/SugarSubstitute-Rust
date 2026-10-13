// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Supply controlled external pipe effects without implementing the owner state machine.
use super::*;
use std::sync::atomic::AtomicU64;
pub(in crate::coordinator_entry::tests) struct Boundary {
    pub(in crate::coordinator_entry::tests) server: bool,
    pub(in crate::coordinator_entry::tests) context_override: Mutex<Option<JobContext>>,
    pub(in crate::coordinator_entry::tests) issue_finished: Gate,
    pub(in crate::coordinator_entry::tests) clock_probe:
        Mutex<Option<(crate::native_deadlines::Role, crate::runtime_test_support::ClockCallback)>>,
    pub(in crate::coordinator_entry::tests) quiesce_probe:
        Mutex<Option<(usize, crate::runtime_test_support::ClockCallback)>>,
    pub(in crate::coordinator_entry::tests) rig_clock:
        Arc<crate::runtime_test_support::ManualClock>,
    pub(in crate::coordinator_entry::tests) signals: Mutex<Vec<[Arc<Signal>; 3]>>,
    incoming: Mutex<VecDeque<(usize, Vec<u8>)>>,
    pub(in crate::coordinator_entry::tests) replies: Mutex<Vec<(protocol::Header, EntryOutcome)>>,
    pub(in crate::coordinator_entry::tests) source: Mutex<CounterDeadline>,
    pub(in crate::coordinator_entry::tests) available: AtomicU64,
    pub(in crate::coordinator_entry::tests) wrong_peer: AtomicBool,
    pub(in crate::coordinator_entry::tests) changed_nonce: AtomicBool,
    pub(in crate::coordinator_entry::tests) fail_prepare: AtomicBool,
    pub(in crate::coordinator_entry::tests) pause_prepare: AtomicBool,
    pub(in crate::coordinator_entry::tests) panic_prepare: AtomicBool,
    pub(in crate::coordinator_entry::tests) fail_retire_once: AtomicBool,
    pub(in crate::coordinator_entry::tests) allow_retire: AtomicBool,
    pub(in crate::coordinator_entry::tests) prepared: Gate,
    pub(in crate::coordinator_entry::tests) retiring: Gate,
    pub(in crate::coordinator_entry::tests) writes: AtomicUsize,
    pub(in crate::coordinator_entry::tests) native_closes: AtomicUsize,
    pub(in crate::coordinator_entry::tests) drops: AtomicUsize,
    pub(in crate::coordinator_entry::tests) initializing: AtomicUsize,
    pub(in crate::coordinator_entry::tests) defer_client_outcome: AtomicBool,
    pub(in crate::coordinator_entry::tests) client_launches: Mutex<Vec<protocol::Header>>,
    pub(in crate::coordinator_entry::tests) memory: Mutex<Option<MemoryLease>>,
    pub(in crate::coordinator_entry::tests) outcome_write_failure:
        Mutex<Option<EntryTransportError>>,
    nonce: AtomicUsize,
    reads: Mutex<Vec<Option<u32>>>,
    pub(in crate::coordinator_entry::tests) pause_role:
        Mutex<Option<crate::native_deadlines::Role>>,
    pub(in crate::coordinator_entry::tests) issue_prepared: Gate,
    pub(in crate::coordinator_entry::tests) issue_release: Gate,
    pub(in crate::coordinator_entry::tests) connects_issued: AtomicUsize,
    pub(in crate::coordinator_entry::tests) connect_failure: Mutex<Option<&'static str>>,
    pub(in crate::coordinator_entry::tests) absent_endpoint: AtomicBool,
    pub(in crate::coordinator_entry::tests) reads_issued: AtomicUsize,
}
impl Boundary {
    pub(in crate::coordinator_entry::tests) fn new(
        rig: &Rig,
        server: bool,
    ) -> TestResult<Arc<Self>> {
        Ok(Arc::new(Self {
            server,
            clock_probe: Mutex::new(None),
            quiesce_probe: Mutex::new(None),
            context_override: Mutex::new(None),
            issue_finished: Gate::default(),
            rig_clock: rig.clock.clone(),
            signals: Mutex::new(Vec::new()),
            incoming: Mutex::new(VecDeque::new()),
            replies: Mutex::new(Vec::new()),
            source: Mutex::new(source()?),
            available: AtomicU64::new(0),
            wrong_peer: AtomicBool::new(false),
            changed_nonce: AtomicBool::new(false),
            fail_prepare: AtomicBool::new(false),
            pause_prepare: AtomicBool::new(false),
            panic_prepare: AtomicBool::new(false),
            fail_retire_once: AtomicBool::new(false),
            allow_retire: AtomicBool::new(true),
            prepared: Gate::default(),
            retiring: Gate::default(),
            writes: AtomicUsize::new(0),
            native_closes: AtomicUsize::new(0),
            drops: AtomicUsize::new(0),
            initializing: AtomicUsize::new(0),
            defer_client_outcome: AtomicBool::new(false),
            client_launches: Mutex::new(Vec::new()),
            memory: Mutex::new(None),
            outcome_write_failure: Mutex::new(None),
            nonce: AtomicUsize::new(0),
            reads: Mutex::new(Vec::new()),
            pause_role: Mutex::new(None),
            issue_prepared: Gate::default(),
            issue_release: Gate::default(),
            connects_issued: AtomicUsize::new(0),
            connect_failure: Mutex::new(None),
            absent_endpoint: AtomicBool::new(false),
            reads_issued: AtomicUsize::new(0),
        }))
    }
    pub(in crate::coordinator_entry::tests) fn connect(&self) -> TestResult {
        self.connect_at(0)
    }
    pub(in crate::coordinator_entry::tests) fn connect_at(&self, index: usize) -> TestResult {
        if index >= MAX_ENTRY_CONNECTIONS {
            return Err("External endpoint index outside fixed capacity".into());
        }
        let original = *lock(&self.source);
        let source =
            CounterDeadline::new(original.ticks() - i64::try_from(index)?, original.frequency())?;
        self.connect_source_at(index, source)
    }
    pub(in crate::coordinator_entry::tests) fn connect_source_at(
        &self,
        index: usize,
        source: CounterDeadline,
    ) -> TestResult {
        if index >= MAX_ENTRY_CONNECTIONS {
            return Err("External endpoint index outside fixed capacity".into());
        }
        let header = protocol::Header {
            namespace: facts()?.namespace(),
            epoch: Some(owner()?.epoch),
            request: EntryRequestId::new(source.ticks() as u64)?,
            source,
            nonce: [0; 32],
        };
        let mut bytes = vec![0; protocol::HEADER_BYTES];
        protocol::encode(&mut bytes, &header, protocol::Payload::Hello)?;
        lock(&self.incoming).push_back((index, bytes));
        self.available.fetch_or(1 << index, Ordering::AcqRel);
        if let Some(signals) = lock(&self.signals).get(index) {
            signals[0].complete();
        }
        Ok(())
    }
    pub(in crate::coordinator_entry::tests) fn wake(&self) {
        if let Some(signals) = lock(&self.signals).first() {
            signals[0].complete();
        }
    }
    fn push(
        &self,
        index: usize,
        header: protocol::Header,
        payload: protocol::Payload<'_>,
    ) -> Result<(), EntryTransportError> {
        let mut bytes = vec![0; MAX_ENTRY_FRAME_BYTES];
        let length = protocol::encode(&mut bytes, &header, payload)?;
        bytes.truncate(length);
        lock(&self.incoming).push_back((index, bytes));
        Ok(())
    }
    fn prepare_issue(
        &self,
        index: usize,
        role: crate::native_deadlines::Role,
    ) -> Result<(), EntryTransportError> {
        if index == 0 && *lock(&self.pause_role) == Some(role) {
            self.issue_prepared.open();
            self.issue_release
                .wait()
                .map_err(|cause| failure_from("entry_fixture_issue_gate", cause))?;
        }
        let probe = {
            let mut stored = lock(&self.clock_probe);
            if index == 0 && stored.as_ref().is_some_and(|(selected, _)| *selected == role) {
                stored.take().map(|(_, probe)| probe)
            } else {
                None
            }
        };
        if let Some(probe) = probe {
            probe();
        }
        Ok(())
    }
}
pub(in crate::coordinator_entry::tests) struct Lease(
    pub(in crate::coordinator_entry::tests) Arc<Boundary>,
);
impl EntryLease for Lease {
    fn sample_counter(&mut self) -> Result<(u64, u64), NativeObservationError> {
        Ok((ClockPort(self.0.rig_clock.clone()).now().as_nanos() + 1000, 1_000_000_000))
    }
    fn prepare(
        &mut self,
        signals: &[[Arc<Signal>; 3]],
        context: &JobContext,
        _until: Moment,
    ) -> Result<(), EntryTransportError> {
        *lock(&self.0.signals) = signals.to_vec();
        *lock(&self.0.reads) = vec![None; signals.len()];
        *lock(&self.0.memory) = Some(context.memory().clone());
        self.0.prepared.open();
        if self.0.pause_prepare.load(Ordering::Acquire) {
            self.0.prepare_issue(0, crate::native_deadlines::Role::Handshake)?;
        }
        if self.0.panic_prepare.load(Ordering::Acquire) {
            // Inject the external unwind only after partial ownership is adopted by the real worker.
            std::panic::resume_unwind(Box::new(
                "Controlled entry boundary prepare panic".to_owned(),
            ));
        }
        if self.0.fail_prepare.load(Ordering::Acquire) {
            return Err(failure("entry_partial_prepare"));
        }
        Ok(())
    }
    fn connect(
        &mut self,
        index: usize,
        issue: NativeIssue<'_>,
    ) -> Result<IssueDispatch<Progress>, EntryTransportError> {
        self.0.prepare_issue(index, crate::native_deadlines::Role::Handshake)?;
        let alternate = lock(&self.0.context_override).clone();
        let issue = NativeIssue { context: alternate.as_ref().unwrap_or(issue.context), ..issue };
        let result = issue.execute(|| {
            self.0.connects_issued.fetch_add(1, Ordering::AcqRel);
            if self.0.absent_endpoint.load(Ordering::Acquire) {
                return Err(EntryTransportError::EndpointAbsent(NativeObservationError::new(
                    "entry_open_client",
                    std::io::Error::from_raw_os_error(2),
                )));
            }
            if let Some(stage) = *lock(&self.0.connect_failure) {
                return Err(failure(stage));
            }
            if !self.0.server || (self.0.available.load(Ordering::Acquire) & (1 << index) != 0) {
                Ok(Progress::Complete(0))
            } else {
                Ok(Progress::Pending)
            }
        });
        self.0.issue_finished.open();
        result
    }
    fn read(
        &mut self,
        index: usize,
        bytes: &mut [u8],
        issue: NativeIssue<'_>,
    ) -> Result<IssueDispatch<Progress>, EntryTransportError> {
        self.0.prepare_issue(index, crate::native_deadlines::Role::Read)?;
        issue.execute(|| {
            self.0.reads_issued.fetch_add(1, Ordering::AcqRel);
            let frame = {
                let mut incoming = lock(&self.0.incoming);
                let position = incoming.iter().position(|(slot, _)| *slot == index);
                position.and_then(|position| incoming.remove(position).map(|(_, bytes)| bytes))
            };
            if let Some(frame) = frame {
                bytes[..frame.len()].copy_from_slice(&frame);
                lock(&self.0.reads)[index] = Some(frame.len() as u32);
                Ok(Progress::Complete(frame.len() as u32))
            } else {
                Ok(Progress::Pending)
            }
        })
    }
    fn write(
        &mut self,
        index: usize,
        bytes: &[u8],
        issue: NativeIssue<'_>,
    ) -> Result<IssueDispatch<Progress>, EntryTransportError> {
        self.0.prepare_issue(0, crate::native_deadlines::Role::Write)?;
        issue.execute(|| {
            self.0.writes.fetch_add(1, Ordering::AcqRel);
            let frame = protocol::decode(bytes)?;
            if matches!(
                frame.payload,
                protocol::DecodedPayload::Accepted(_) | protocol::DecodedPayload::Rejected(_)
            ) && let Some(error) = lock(&self.0.outcome_write_failure).take()
            {
                return Err(error);
            }
            match frame.payload {
                protocol::DecodedPayload::Challenge if self.0.server => {
                    let mut header = frame.header;
                    if self.0.changed_nonce.load(Ordering::Acquire) {
                        header.nonce[0] ^= 1;
                    }
                    let request =
                        request().map_err(|cause| failure_from("entry_fixture_request", cause))?;
                    self.0.push(index, header, protocol::Payload::Launch(&request))?;
                }
                protocol::DecodedPayload::Hello if !self.0.server => {
                    let mut header = frame.header;
                    if self
                        .0
                        .initializing
                        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                            count.checked_sub(1)
                        })
                        .is_ok()
                    {
                        header.epoch = None;
                        self.0.push(index, header, protocol::Payload::Initializing)?;
                    } else {
                        header.epoch = Some(
                            owner()
                                .map_err(|cause| failure_from("entry_fixture_owner", cause))?
                                .epoch,
                        );
                        let nonce = self.0.nonce.fetch_add(1, Ordering::AcqRel) + 1;
                        header.nonce =
                            [u8::try_from(nonce).map_err(|_| EntryTransportError::Capacity)?; 32];
                        self.0.push(index, header, protocol::Payload::Challenge)?;
                    }
                }
                protocol::DecodedPayload::Launch(_) if !self.0.server => {
                    lock(&self.0.client_launches).push(frame.header);
                    if !self.0.defer_client_outcome.load(Ordering::Acquire) {
                        self.0.push(
                            index,
                            frame.header,
                            protocol::Payload::Accepted(
                                run().map_err(|cause| failure_from("entry_fixture_run", cause))?,
                            ),
                        )?;
                    }
                }
                protocol::DecodedPayload::Accepted(run) => {
                    lock(&self.0.replies).push((frame.header, EntryOutcome::Accepted(run)))
                }
                protocol::DecodedPayload::Rejected(reason) => {
                    lock(&self.0.replies).push((frame.header, EntryOutcome::Rejected(reason)))
                }
                protocol::DecodedPayload::Initializing => {}
                _ => return Err(EntryTransportError::InvalidPhase),
            }
            Ok(Progress::Complete(bytes.len() as u32))
        })
    }
    fn poll(
        &mut self,
        index: usize,
        role: crate::native_deadlines::Role,
    ) -> Result<Progress, EntryTransportError> {
        match role {
            crate::native_deadlines::Role::Handshake => Ok(
                if !self.0.server || (self.0.available.load(Ordering::Acquire) & (1 << index) != 0)
                {
                    Progress::Complete(0)
                } else {
                    Progress::Pending
                },
            ),
            crate::native_deadlines::Role::Read => {
                Ok(lock(&self.0.reads)[index].map_or(Progress::Pending, Progress::Complete))
            }
            crate::native_deadlines::Role::Write => Err(EntryTransportError::InvalidPhase),
        }
    }
    fn quiesce(
        &mut self,
        index: usize,
        role: crate::native_deadlines::Role,
    ) -> Result<(), EntryTransportError> {
        if role == crate::native_deadlines::Role::Read {
            lock(&self.0.reads)[index] = None;
            let probe = {
                let mut stored = lock(&self.0.quiesce_probe);
                if index == 0
                    && stored.as_ref().is_some_and(|(read, _)| {
                        *read == self.0.reads_issued.load(Ordering::Acquire)
                    })
                {
                    stored.take().map(|(_, probe)| probe)
                } else {
                    None
                }
            };
            if let Some(probe) = probe {
                probe();
            }
        }
        Ok(())
    }
    fn verify(&mut self, _index: usize, _message: bool) -> Result<KernelPeer, EntryTransportError> {
        let mut facts = facts().map_err(|cause| failure_from("entry_fixture_facts", cause))?;
        if self.0.wrong_peer.load(Ordering::Acquire) {
            facts = EntryNativeIdentityFacts::new(
                facts.namespace(),
                facts.account_sid(),
                2,
                facts.host_image_identity(),
            )
            .map_err(EntryTransportError::Shape)?;
        }
        Ok(KernelPeer { facts, pid: 17, birth: 23 })
    }
    fn nonce(&mut self) -> Result<[u8; 32], EntryTransportError> {
        let value = self.0.nonce.fetch_add(1, Ordering::AcqRel) + 1;
        Ok([u8::try_from(value).map_err(|_| EntryTransportError::Capacity)?; 32])
    }
    fn retire_connection(
        &mut self,
        index: usize,
        _reuse: bool,
    ) -> Result<bool, EntryTransportError> {
        if !self.0.allow_retire.load(Ordering::Acquire) {
            self.0.retiring.open();
            return Ok(false);
        }
        self.0.available.fetch_and(!(1 << index), Ordering::AcqRel);
        self.0.native_closes.fetch_add(1, Ordering::AcqRel);
        Ok(true)
    }
    fn retire(&mut self) -> Result<bool, EntryTransportError> {
        self.0.retiring.open();
        if self.0.fail_retire_once.swap(false, Ordering::AcqRel) {
            self.0.wake();
            return Err(failure("entry_partial_retirement"));
        }
        Ok(self.0.allow_retire.load(Ordering::Acquire))
    }
}
fn failure_from(stage: &'static str, cause: Box<dyn std::error::Error>) -> EntryTransportError {
    failure_with_text(stage, cause.to_string())
}
fn failure_with_text(stage: &'static str, cause: String) -> EntryTransportError {
    EntryTransportError::native(NativeObservationError::new(stage, std::io::Error::other(cause)))
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.0.drops.fetch_add(1, Ordering::AcqRel);
    }
}
