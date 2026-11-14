// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Run the window's and the coordinator's lifecycle owners as a pair, each on its own thread,
//! exchanging encoded lifecycle frames.
//!
//! Each peer owns its real lifecycle owners on a thread of its own, the one blocking worker of
//! a host execution owner standing in for its process, and is driven in lock-step turns by the
//! test thread. Between the peers only bytes travel: every frame is encoded with
//! [`FrameCodec::encode`] by its sender and decoded with [`FrameCodec::decode`] by its receiver,
//! and every frame is recorded in an exchange log that a failing test prints. Two facts the
//! native layer reports outside the pipe travel beside the frames: the window process's exit
//! code, and the coordinator terminating a window it cut off.
//!
//! Time is virtual and moves only when nothing else can: when neither peer changed, no frame
//! is in flight and no peer awaits running executor work, the pair advances to the earliest
//! deadline either peer reports, exactly as the machine would by waiting. A pair with nothing
//! due and nothing moving is stuck, and the test fails at once with the exchange log; no test
//! passes or fails because wall-clock time elapsed. Executor work is awaited through the
//! peers' shared completion wake, whose waits fail only on the process-wide no-progress guard.
use crate::completion_wake::{Wake, wait_while_watched};
use std::{
    collections::VecDeque,
    error::Error,
    fmt::Write as _,
    sync::{
        Arc, Condvar, Mutex, PoisonError,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use sugarsubstitute_contracts::execution::{
    ExecutionPort, JobClass, JobId, JobSpec, Moment, Operation, OwnedJob, Scope, ScopeId, ScopeSpec,
};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};
use sugarsubstitute_supervision::lifecycle_protocol::{
    DecodedPayload, FrameCodec, MAX_FRAME_BYTES, ReplyOutcome, WireMessage,
};

pub(crate) type HarnessResult<T = ()> = Result<T, Box<dyn Error>>;

/// Turns the pair may take before it is judged to cycle without settling.
const ROUND_LIMIT: usize = 20_000;

/// Which peer an exchange came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Side {
    Window,
    Coordinator,
}
impl Side {
    const fn name(self) -> &'static str {
        match self {
            Self::Window => "window",
            Self::Coordinator => "coordinator",
        }
    }
}

/// What reaches a peer: an encoded frame, or a native fact reported beside the pipe.
#[derive(Debug)]
pub(crate) enum Delivery {
    /// One complete encoded lifecycle frame.
    Frame(Vec<u8>),
    /// The window process exited with this code.
    Exited(u32),
    /// The coordinator's role cleanup terminated the window process.
    Terminated,
}

/// The shared exchange log and the virtual time it is stamped with.
#[derive(Clone, Default)]
pub(crate) struct Log(Arc<Mutex<LogState>>);
#[derive(Default)]
struct LogState {
    now: Duration,
    lines: Vec<String>,
}
impl Log {
    fn state(&self) -> std::sync::MutexGuard<'_, LogState> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
    fn set_now(&self, now: Duration) {
        self.state().now = now;
    }
    /// Record one line at the current virtual time.
    pub(crate) fn note(&self, line: impl AsRef<str>) {
        let mut state = self.state();
        let stamp = format!("[+{:>6} ms] {}", state.now.as_millis(), line.as_ref());
        state.lines.push(stamp);
    }
    /// Every line recorded so far, one per row.
    pub(crate) fn render(&self) -> String {
        let state = self.state();
        let mut text = String::new();
        for line in &state.lines {
            let _ = writeln!(text, "{line}");
        }
        text
    }
    /// Whether any recorded line contains `needle`.
    pub(crate) fn contains(&self, needle: &str) -> bool {
        self.state().lines.iter().any(|line| line.contains(needle))
    }
}

/// Name a decoded frame for the exchange log, without its secret or bulky payload bytes.
pub(crate) fn describe(bytes: &[u8]) -> String {
    let Ok(frame) = FrameCodec::decode(bytes) else {
        return "malformed frame".to_owned();
    };
    let request = frame.header.request.map_or(0, |request| request.get());
    let what = match frame.payload {
        DecodedPayload::ExitIntent { shutdown, intent, expected_code } => {
            format!(
                "ExitIntent {{ shutdown: {}, intent: {intent:?}, code: {expected_code} }}",
                shutdown.get()
            )
        }
        DecodedPayload::ExitCompleted(shutdown) => format!("ExitCompleted({})", shutdown.get()),
        DecodedPayload::RestartToApply => "RestartToApply".to_owned(),
        DecodedPayload::CheckForUpdates => "CheckForUpdates".to_owned(),
        DecodedPayload::CloseRequest => "CloseRequest".to_owned(),
        DecodedPayload::MainRevealed(_) => "MainRevealed".to_owned(),
        DecodedPayload::Reply(ReplyOutcome::Acknowledged) => "Reply(Acknowledged)".to_owned(),
        DecodedPayload::Reply(other) => format!("Reply({other:?})"),
        DecodedPayload::UpdateStatus { revision, workflow, status } => {
            match sugarsubstitute_contracts::update::status::UpdateStatus::decode(status) {
                Ok(decoded) => format!(
                    "UpdateStatus {{ revision: {revision}, workflow: {workflow:?}, restarting: {}, applying: {:?}, restart_covered: {} }}",
                    decoded.restarting(),
                    decoded.applying(),
                    decoded.restart_covered()
                ),
                Err(cause) => {
                    format!("UpdateStatus {{ revision: {revision}, undecodable: {cause} }}")
                }
            }
        }
        other => format!("{other:?}").chars().take(80).collect(),
    };
    if request == 0 { what } else { format!("{what} [request {request}]") }
}

/// A first-in first-out queue another thread may wait on.
struct Mailbox<T> {
    state: Mutex<(VecDeque<T>, bool)>,
    changed: Condvar,
}
impl<T> Mailbox<T> {
    fn new() -> Arc<Self> {
        Arc::new(Self { state: Mutex::new((VecDeque::new(), false)), changed: Condvar::new() })
    }
    fn state(&self) -> std::sync::MutexGuard<'_, (VecDeque<T>, bool)> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
    fn push(&self, value: T) {
        self.state().0.push_back(value);
        self.changed.notify_all();
    }
    fn close(&self) {
        self.state().1 = true;
        self.changed.notify_all();
    }
    fn try_pop(&self) -> Option<T> {
        self.state().0.pop_front()
    }
    /// Wait for the next value; `None` once the queue is closed and empty.
    fn pop(&self, what: &'static str) -> Option<T> {
        let mut state = wait_while_watched(
            &self.changed,
            self.state(),
            || what.to_owned(),
            |(queue, closed)| queue.is_empty() && !*closed,
        );
        state.0.pop_front()
    }
}

/// One peer's end of the pipe: what it sends is encoded, logged and counted in flight.
pub(crate) struct WireEnd {
    side: Side,
    outgoing: Arc<Mailbox<Delivery>>,
    incoming: Arc<Mailbox<Delivery>>,
    in_flight: Arc<AtomicUsize>,
    log: Log,
}
impl WireEnd {
    /// Encode `message` with the protocol codec and send its bytes to the other peer.
    pub(crate) fn send(&self, message: &WireMessage<'_>) -> HarnessResult {
        let mut buffer = vec![0; MAX_FRAME_BYTES];
        let length = FrameCodec::encode(message, &mut buffer)?;
        buffer.truncate(length);
        self.log.note(format!(
            "{} -> {}: {}",
            self.side.name(),
            self.other().name(),
            describe(&buffer)
        ));
        self.deliver(Delivery::Frame(buffer))
    }
    /// Report a native fact beside the pipe.
    pub(crate) fn signal(&self, delivery: Delivery) -> HarnessResult {
        self.log.note(format!(
            "{} -> {}: <native> {delivery:?}",
            self.side.name(),
            self.other().name()
        ));
        self.deliver(delivery)
    }
    fn deliver(&self, delivery: Delivery) -> HarnessResult {
        self.in_flight.fetch_add(1, Ordering::AcqRel);
        self.outgoing.push(delivery);
        Ok(())
    }
    /// Take the next delivery without waiting.
    pub(crate) fn receive(&self) -> Option<Delivery> {
        let delivery = self.incoming.try_pop()?;
        self.in_flight.fetch_sub(1, Ordering::AcqRel);
        Some(delivery)
    }
    /// The shared exchange log.
    pub(crate) fn log(&self) -> &Log {
        &self.log
    }
    const fn other(&self) -> Side {
        match self.side {
            Side::Window => Side::Coordinator,
            Side::Coordinator => Side::Window,
        }
    }
}

/// Connect two pipe ends that share one exchange log.
pub(crate) fn pipe(log: &Log) -> (WireEnd, WireEnd, Arc<AtomicUsize>) {
    let window_inbox = Mailbox::new();
    let coordinator_inbox = Mailbox::new();
    let (to_window, to_coordinator) = (window_inbox.clone(), coordinator_inbox.clone());
    let in_flight = Arc::new(AtomicUsize::new(0));
    let window = WireEnd {
        side: Side::Window,
        outgoing: to_coordinator,
        incoming: window_inbox,
        in_flight: in_flight.clone(),
        log: log.clone(),
    };
    let coordinator = WireEnd {
        side: Side::Coordinator,
        outgoing: to_window,
        incoming: coordinator_inbox,
        in_flight: in_flight.clone(),
        log: log.clone(),
    };
    (window, coordinator, in_flight)
}

/// What one peer turn observed.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Turn {
    /// The peer received or sent something, or its observable state changed.
    pub(crate) progressed: bool,
    /// Executor work the peer awaits is still running.
    pub(crate) busy: bool,
    /// The earliest virtual time at which the peer's own deadline falls due.
    pub(crate) due: Option<Duration>,
}

/// A peer the pair drives in turns at a virtual time.
pub(crate) trait LifecyclePeer {
    /// Take one turn at virtual time `now`.
    /// # Errors
    /// Returns a failure of the peer's owners; the pair stops and reports it with the log.
    fn turn(&mut self, now: Duration) -> HarnessResult<Turn>;
}

type Command<P> = Box<dyn FnOnce(&mut P) + Send>;

/// A peer built and owned on its own thread: the one blocking job of a host execution owner
/// standing in for the peer's process. The test thread only sends it closures.
pub(crate) struct PeerThread<P> {
    commands: Arc<Mailbox<Command<P>>>,
    job: Option<OwnedJob<(), String>>,
    scope: Option<Scope>,
    client: ExecutionClient,
    owner: Option<ExecutionOwner>,
    wake: Arc<Wake>,
    name: &'static str,
}
impl<P: 'static> PeerThread<P> {
    /// Build the peer with `build` on its own thread, called `name` in failures.
    /// # Errors
    /// Returns the builder's failure.
    pub(crate) fn spawn(
        name: &'static str,
        build: impl FnOnce() -> Result<P, String> + Send + 'static,
    ) -> HarnessResult<Self> {
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::new(crate::paused_clock::PausedClock::new(), wake.clone())?;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let commands = Mailbox::<Command<P>>::new();
        let built = Mailbox::<Result<(), String>>::new();
        let (inbox, report) = (commands.clone(), built.clone());
        let spec = JobSpec {
            id: JobId::new(1)?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: None,
            reservation_bytes: 1024 * 1024,
            replacement: None,
        };
        let job = client.submit_blocking(&scope, spec, move |_| {
            let mut peer = match build() {
                Ok(peer) => {
                    report.push(Ok(()));
                    peer
                }
                Err(cause) => {
                    report.push(Err(cause.clone()));
                    return Err(cause);
                }
            };
            while let Some(command) = inbox.pop("the test's next command") {
                command(&mut peer);
            }
            Ok(())
        })?;
        let peer = Self {
            commands,
            job: Some(job),
            scope: Some(scope),
            client,
            owner: Some(owner),
            wake,
            name,
        };
        match built.pop("the peer to be built") {
            Some(Ok(())) => Ok(peer),
            Some(Err(cause)) => Err(format!("{name} could not be built: {cause}").into()),
            None => Err(format!("{name} stopped while it was built").into()),
        }
    }
    /// Run `command` on the peer's thread and return its result.
    /// # Errors
    /// Returns a failure when the peer's thread stopped before answering.
    pub(crate) fn call<R: Send + 'static>(
        &mut self,
        command: impl FnOnce(&mut P) -> R + Send + 'static,
    ) -> HarnessResult<R> {
        let reply = Mailbox::<R>::new();
        let answer = reply.clone();
        /// Closes the reply when the command ends, answered or unwinding.
        struct CloseOnDrop<R>(Arc<Mailbox<R>>);
        impl<R> Drop for CloseOnDrop<R> {
            fn drop(&mut self) {
                self.0.close();
            }
        }
        self.commands.push(Box::new(move |peer: &mut P| {
            let closing = CloseOnDrop(answer);
            closing.0.push(command(peer));
        }));
        reply
            .pop("the peer's answer")
            .ok_or_else(|| format!("{} stopped without answering", self.name).into())
    }
    /// Run a fallible `command` on the peer's thread.
    /// # Errors
    /// Returns the command's failure, or a failure when the peer's thread stopped.
    pub(crate) fn act(
        &mut self,
        command: impl FnOnce(&mut P) -> HarnessResult + Send + 'static,
    ) -> HarnessResult {
        self.call(move |peer| command(peer).map_err(|cause| cause.to_string()))?.map_err(Into::into)
    }
}
impl<P> Drop for PeerThread<P> {
    fn drop(&mut self) {
        self.commands.close();
        if let Some(mut job) = self.job.take() {
            let _ = self.wake.completion(&self.client, &mut job);
        }
        drop(self.scope.take());
        if let Some(mut owner) = self.owner.take() {
            let _ = self.wake.finish_shutdown(&mut owner, &self.client, Moment::from_nanos(1));
        }
    }
}
/// The two peers, their shared completion wake, the exchange log and virtual time.
pub(crate) struct Pair<W: LifecyclePeer + 'static, C: LifecyclePeer + 'static> {
    pub(crate) window: PeerThread<W>,
    pub(crate) coordinator: PeerThread<C>,
    wake: Arc<Wake>,
    in_flight: Arc<AtomicUsize>,
    log: Log,
    now: Duration,
}
impl<W: LifecyclePeer + 'static, C: LifecyclePeer + 'static> Pair<W, C> {
    pub(crate) fn new(
        window: PeerThread<W>,
        coordinator: PeerThread<C>,
        wake: Arc<Wake>,
        in_flight: Arc<AtomicUsize>,
        log: Log,
    ) -> Self {
        Self { window, coordinator, wake, in_flight, log, now: Duration::ZERO }
    }

    /// The exchange log.
    pub(crate) fn log(&self) -> &Log {
        &self.log
    }

    /// Take turns until `done` holds, advancing virtual time only while nothing moves.
    /// # Errors
    /// Fails with the exchange log when a peer fails, when both peers wait with nothing due,
    /// or when the pair cycles without settling.
    pub(crate) fn run_until(
        &mut self,
        what: &str,
        mut done: impl FnMut(&mut Self) -> HarnessResult<bool>,
    ) -> HarnessResult {
        self.run(what, None, &mut done)
    }

    /// Take turns until virtual time reaches `until`, passing through every deadline on the way.
    /// # Errors
    /// Fails as [`Self::run_until`] does.
    pub(crate) fn run_for(&mut self, what: &str, duration: Duration) -> HarnessResult {
        let until = self.now.saturating_add(duration);
        self.run(what, Some(until), &mut |pair: &mut Self| Ok(pair.now >= until))
    }

    fn run(
        &mut self,
        what: &str,
        limit: Option<Duration>,
        done: &mut dyn FnMut(&mut Self) -> HarnessResult<bool>,
    ) -> HarnessResult {
        self.log.note(format!("--- {what}"));
        for _ in 0..ROUND_LIMIT {
            if done(self)? {
                return Ok(());
            }
            let revision = self.wake.revision();
            let now = self.now;
            let coordinator = self
                .coordinator
                .call(move |peer| peer.turn(now).map_err(|cause| cause.to_string()))?;
            let coordinator = coordinator
                .map_err(|cause| self.failure(what, &format!("the coordinator failed: {cause}")))?;
            let window =
                self.window.call(move |peer| peer.turn(now).map_err(|cause| cause.to_string()))?;
            let window = window
                .map_err(|cause| self.failure(what, &format!("the window failed: {cause}")))?;
            // A completion wake delivered during the round owes the peers a turn, as the native
            // loop's completion event does, before time may pass.
            let woken = self.wake.revision() != revision;
            if woken
                || coordinator.progressed
                || window.progressed
                || self.in_flight.load(Ordering::Acquire) > 0
            {
                continue;
            }
            if coordinator.busy || window.busy {
                self.wake.wait_after(revision);
                continue;
            }
            let due = [coordinator.due, window.due]
                .into_iter()
                .flatten()
                .filter(|due| *due > now)
                .min()
                .map(|due| limit.map_or(due, |limit| due.min(limit)))
                .or(limit.filter(|limit| *limit > now));
            let Some(due) = due else {
                return Err(self.failure(what, "both peers wait and nothing is due"));
            };
            self.now = due;
            self.log.set_now(due);
            self.log.note("nothing moves; time passes to this moment");
        }
        Err(self.failure(what, "the peers kept exchanging without settling"))
    }

    fn failure(&self, what: &str, reason: &str) -> Box<dyn Error> {
        format!("{what}: {reason}\nexchange log:\n{}", self.log.render()).into()
    }
}
