// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Find out whether a ComfyUI the application did not start holds its backend port, and close
//! it when the user decides so.
//!
//! The application runs and controls its own ComfyUI. A ComfyUI started some other way on the
//! same port answers in its place, and the application's start then fails because the process
//! that answered is not its own. This owner asks the listener who it is: it reads
//! `/system_stats` over one established connection, identifies the process serving that exact
//! connection, and retains it by identifier and creation time. Only a listener that answered
//! as ComfyUI, and that no copy of this application started, is offered for closing.
use std::{
    error::Error,
    fmt,
    sync::{Arc, Mutex, PoisonError},
};
use sugarsubstitute_comfy::readiness::{MAX_SYSTEM_STATS_BYTES, decode_system_stats};
use sugarsubstitute_contracts::{
    execution::JobContext,
    instance_conflict::{ConflictBackend, ConflictObservation, ConflictOperation},
};
use sugarsubstitute_networking::Transport;
use sugarsubstitute_supervision::{
    backend_listener::{
        AttachedListenerLease, ConnectedLoopback, TCP_OWNER_WORKSPACE_BYTES, TcpOwnerWorkspace,
    },
    foreign_backend::{ForeignBackend, ForeignBackendOwner},
};

/// The file name of this application's own images.
const APPLICATION_IMAGE: &str = "sugarsubstitute.exe";

/// The memory one operation's job reserves: the kernel connection table's workspace and the
/// retained process list, with room for the listener's answer, the peer's handle and the
/// transport's buffers. A job that reserves less cannot identify any listener.
pub(crate) const RESERVATION_BYTES: u64 =
    TCP_OWNER_WORKSPACE_BYTES + ForeignBackend::RETAIN_BYTES + 2 * 1024 * 1024;

/// What asking the listener found.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Finding {
    /// Nothing has been asked yet.
    Unknown,
    /// Nothing accepted a connection on the port.
    NotListening,
    /// The listener belongs to a running copy of this application.
    Application,
    /// A ComfyUI nothing of this application started.
    Foreign,
    /// Something listens, and it could not be shown to be ComfyUI.
    Unverified,
}

/// Why resolving the listener failed, retaining the original cause.
#[derive(Debug)]
pub(crate) struct BackendConflictError {
    stage: &'static str,
    cause: Option<Box<dyn Error + Send + Sync>>,
}

impl BackendConflictError {
    fn new(stage: &'static str, cause: impl Error + Send + Sync + 'static) -> Self {
        Self { stage, cause: Some(Box::new(cause)) }
    }
    fn invalid(stage: &'static str) -> Self {
        Self { stage, cause: None }
    }
}

impl fmt::Display for BackendConflictError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Backend port conflict failed at {}", self.stage)
    }
}

impl Error for BackendConflictError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause.as_deref().map(|cause| cause as &(dyn Error + 'static))
    }
}

struct State {
    port: u16,
    finding: Finding,
    foreign: ForeignBackend,
    retired: bool,
}

/// Resolve one port's listener; clones share the same retained process.
#[derive(Clone)]
pub(crate) struct BackendPortSession {
    state: Arc<Mutex<State>>,
}

impl BackendPortSession {
    /// Hold nothing until discovery runs in an admitted job.
    pub(crate) fn new(port: u16) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                port,
                finding: Finding::Unknown,
                foreign: ForeignBackend::new(),
                retired: false,
            })),
        }
    }

    /// What discovery found; `Unknown` until it has run.
    pub(crate) fn finding(&self) -> Finding {
        self.state.lock().unwrap_or_else(PoisonError::into_inner).finding
    }
}

impl ConflictBackend for BackendPortSession {
    type Error = BackendConflictError;

    fn execute(
        &self,
        operation: ConflictOperation,
        context: &JobContext,
    ) -> Result<ConflictObservation, BackendConflictError> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        match operation {
            ConflictOperation::Retire => {
                state
                    .foreign
                    .retire()
                    .map_err(|cause| BackendConflictError::new("retire", cause))?;
                state.retired = true;
                Ok(ConflictObservation::Retired)
            }
            _ if state.retired => Err(BackendConflictError::invalid("retired")),
            ConflictOperation::Discover => discover(&mut state, context),
            ConflictOperation::Observe => observe(&state),
            ConflictOperation::Close | ConflictOperation::Force => {
                if observe(&state)? == ConflictObservation::Released {
                    return Ok(ConflictObservation::Released);
                }
                state
                    .foreign
                    .close(context)
                    .map_err(|cause| BackendConflictError::new("close", cause))?;
                Ok(ConflictObservation::Waiting)
            }
        }
    }

    /// Nothing is launched: the caller starts its own backend again once the port is free.
    fn relaunch(&self, _context: &JobContext) -> Result<(), BackendConflictError> {
        Ok(())
    }

    fn retire_relaunch(&self) -> Result<(), BackendConflictError> {
        Ok(())
    }
}

fn observe(state: &State) -> Result<ConflictObservation, BackendConflictError> {
    if state.finding != Finding::Foreign {
        return Err(BackendConflictError::invalid("observe_without_owner"));
    }
    Ok(if state.foreign.exited().map_err(|cause| BackendConflictError::new("observe", cause))? {
        ConflictObservation::Released
    } else {
        ConflictObservation::Waiting
    })
}

/// Ask the listener who it is, then retain it. Every failure after a connection was accepted
/// leaves the listener unverified and untouched.
fn discover(
    state: &mut State,
    context: &JobContext,
) -> Result<ConflictObservation, BackendConflictError> {
    if state.finding != Finding::Unknown {
        return Err(BackendConflictError::invalid("repeated_discovery"));
    }
    let found = resolve(state, context);
    match &found {
        Ok(_) => tracing::info!(
            operation = "backend_port_discovery",
            port = state.port,
            finding = ?state.finding,
            "A ComfyUI the application did not start holds its port"
        ),
        Err(error) => tracing::info!(
            operation = "backend_port_discovery",
            port = state.port,
            finding = ?state.finding,
            stage = error.stage,
            cause = error.cause.as_ref().map(tracing::field::display),
            "The listener on the application's port was resolved"
        ),
    }
    found
}

/// Set `state.finding` from the listener's answer and the process that gave it.
fn resolve(
    state: &mut State,
    context: &JobContext,
) -> Result<ConflictObservation, BackendConflictError> {
    let mut http = Transport::default();
    let asked = ask(&mut http, state.port, context);
    let closed = http.close();
    let peer = match asked {
        Ok(peer) => peer,
        Err(Asked::NotListening(cause)) => {
            state.finding = Finding::NotListening;
            return Err(cause);
        }
        Err(Asked::Unverified(cause)) => {
            state.finding = Finding::Unverified;
            return Err(cause);
        }
    };
    if let Err(cause) = closed {
        state.finding = Finding::Unverified;
        return Err(BackendConflictError::new("close_connection", cause));
    }
    match state.foreign.retain(peer, APPLICATION_IMAGE, context) {
        Ok(ForeignBackendOwner::Foreign(image)) => {
            state.finding = Finding::Foreign;
            Ok(ConflictObservation::Owner(image))
        }
        Ok(ForeignBackendOwner::Application) => {
            state.finding = Finding::Application;
            Err(BackendConflictError::invalid("listener_is_the_application"))
        }
        Err(cause) => {
            state.finding = Finding::Unverified;
            Err(BackendConflictError::new("retain", cause))
        }
    }
}

enum Asked {
    NotListening(BackendConflictError),
    Unverified(BackendConflictError),
}

/// Record a failure met after the listener accepted the connection.
fn unverified<E: Error + Send + Sync + 'static>(stage: &'static str) -> impl FnOnce(E) -> Asked {
    move |cause| Asked::Unverified(BackendConflictError::new(stage, cause))
}

/// Read `/system_stats` from the port and identify the process that served that connection.
fn ask(
    http: &mut Transport,
    port: u16,
    context: &JobContext,
) -> Result<sugarsubstitute_supervision::backend_listener::BackendPeerIdentity, Asked> {
    http.connect_loopback(port, context)
        .map_err(|cause| Asked::NotListening(BackendConflictError::new("connect", cause)))?;
    http.request_loopback("/system_stats", context)
        .map_err(|cause| Asked::NotListening(BackendConflictError::new("request", cause)))?;
    http.send_loopback(context)
        .map_err(|cause| Asked::NotListening(BackendConflictError::new("send", cause)))?;
    http.receive(context).map_err(unverified("receive"))?;
    let (local, remote) = http.loopback_tuple(port, context).map_err(unverified("tuple"))?;
    let connection = ConnectedLoopback::new(local, remote).map_err(unverified("connection"))?;
    let mut table = TcpOwnerWorkspace::new(context).map_err(unverified("table"))?;
    let lease = AttachedListenerLease::new();
    let peer = lease.qualify_connection(connection, &mut table, context);
    let released = lease.close(context);
    let peer = peer.map_err(unverified("peer"))?;
    released.map_err(unverified("peer_release"))?;
    let response = http.response(context).map_err(unverified("response"))?;
    if response.status != 200 {
        return Err(Asked::Unverified(BackendConflictError::invalid("status")));
    }
    let mut body = Vec::new();
    let mut scratch = [0_u8; 16 * 1024];
    loop {
        let read = http.read(&mut scratch, context).map_err(unverified("read"))?;
        if read == 0 {
            break;
        }
        if body.len() + read > MAX_SYSTEM_STATS_BYTES {
            return Err(Asked::Unverified(BackendConflictError::invalid("body_bound")));
        }
        body.extend_from_slice(&scratch[..read]);
    }
    decode_system_stats(&body).map_err(unverified("not_comfyui"))?;
    http.close_request().map_err(unverified("close_request"))?;
    Ok(peer)
}

#[cfg(test)]
mod tests {
    use super::{MAX_SYSTEM_STATS_BYTES, RESERVATION_BYTES};
    use sugarsubstitute_supervision::{
        backend_listener::TCP_OWNER_WORKSPACE_BYTES, foreign_backend::ForeignBackend,
    };

    #[test]
    fn a_job_reserves_enough_to_read_the_connection_table_and_retain_the_listener() {
        // The peer's handle is charged 64 KiB by the listener lease.
        let claimed = TCP_OWNER_WORKSPACE_BYTES
            + ForeignBackend::RETAIN_BYTES
            + 64 * 1024
            + MAX_SYSTEM_STATS_BYTES as u64;
        assert!(RESERVATION_BYTES >= claimed + 1024 * 1024);
    }
}
