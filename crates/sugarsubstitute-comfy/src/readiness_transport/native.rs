// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind shared bounded transport to listener identity and feature-owned response verification.
use super::{
    owner::{HttpPort, Transaction, checkpoint},
    probe::Endpoint,
    types::Listener,
    *,
};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_supervision::backend_listener::ConnectedLoopback;
/// Apply readiness listener identity and body policy around shared native request custody.
pub(super) struct SystemPort;
pub(super) mod retry;
impl HttpPort for SystemPort {
    fn begin(
        &self,
        state: &mut Transaction,
        port: u16,
        context: &JobContext,
    ) -> Result<(), ReadinessTransportError> {
        checkpoint(context)?;
        if state.peer.is_none()
            && !sugarsubstitute_supervision::backend_listener::loopback_listener_present(
                port,
                &mut state.tcp,
                context,
            )
            .map_err(|cause| ReadinessTransportError::new("http_listener_query", cause))?
        {
            return Err(ReadinessTransportError::unavailable(
                "http_listener_absent",
                std::io::Error::from(std::io::ErrorKind::ConnectionRefused),
            ));
        }
        checkpoint(context)?;
        state
            .http
            .readiness(port, context)
            .map_err(|e| retry::transport(e, state.peer.is_some(), context))
    }
    fn get(
        &self,
        state: &mut Transaction,
        input: &ReadinessProbeInput,
        endpoint: Endpoint<'_>,
        context: &JobContext,
        consume: &mut dyn FnMut(&[u8]) -> Result<(), ReadinessTransportError>,
    ) -> Result<(), ReadinessTransportError> {
        checkpoint(context)?;
        state
            .http
            .readiness_request(endpoint.path(), context)
            .map_err(|e| retry::transport(e, state.peer.is_some(), context))?;
        state
            .http
            .send_readiness(context)
            .map_err(|e| retry::transport(e, state.peer.is_some(), context))?;
        state
            .http
            .receive(context)
            .map_err(|e| retry::transport(e, state.peer.is_some(), context))?;
        let before = connection(state, input.port(), context)?;
        qualify(state, input, before, context)?;
        let response = state
            .http
            .response(context)
            .map_err(|e| retry::transport(e, state.peer.is_some(), context))?;
        if response.status != 200 {
            return Err(ReadinessTransportError::invalid("http_status_not_200"));
        }
        let declared = super::headers::parse_headers(&response.headers, endpoint)?;
        let mut scratch = [0u8; 32 * 1024];
        let mut total = 0u64;
        loop {
            let read = state
                .http
                .read(&mut scratch, context)
                .map_err(|e| retry::transport(e, state.peer.is_some(), context))?;
            if read == 0 {
                break;
            }
            total = total
                .checked_add(read as u64)
                .filter(|n| *n <= endpoint.maximum())
                .ok_or_else(|| ReadinessTransportError::invalid("http_body_bound"))?;
            consume(&scratch[..read])?;
        }
        if declared.is_some_and(|length| length != total) {
            return Err(ReadinessTransportError::invalid("http_truncated_or_excess_body"));
        }
        let after = connection(state, input.port(), context)?;
        if after != before {
            return Err(ReadinessTransportError::invalid("http_connected_tuple_changed"));
        }
        qualify(state, input, after, context)?;
        checkpoint(context)?;
        state
            .http
            .close_request()
            .map_err(|e| retry::transport(e, state.peer.is_some(), context))?;
        checkpoint(context)
    }
    fn close(
        &self,
        state: &mut Transaction,
        context: &JobContext,
    ) -> Result<(), ReadinessTransportError> {
        checkpoint(context)?;
        state.http.close().map_err(|e| retry::transport(e, state.peer.is_some(), context))?;
        checkpoint(context)
    }
}
/// Adapt observed native tuple facts into the process-qualification boundary without granting trust.
fn connection(
    state: &mut Transaction,
    port: u16,
    context: &JobContext,
) -> Result<ConnectedLoopback, ReadinessTransportError> {
    let (local, remote) = state
        .http
        .loopback_tuple(port, context)
        .map_err(|e| retry::transport(e, state.peer.is_some(), context))?;
    ConnectedLoopback::new(local, remote)
        .map_err(|e| ReadinessTransportError::new("http_connected_tuple", e))
}
fn qualify(
    state: &mut Transaction,
    input: &ReadinessProbeInput,
    connection: ConnectedLoopback,
    context: &JobContext,
) -> Result<(), ReadinessTransportError> {
    let peer = match &input.listener {
        Listener::Managed(lease, _) => {
            lease.qualify_connection(connection, &mut state.tcp, context)
        }
        Listener::Attached(lease, _) => {
            lease.qualify_connection(connection, &mut state.tcp, context)
        }
    }
    .map_err(|e| ReadinessTransportError::new("http_native_peer", e))?;
    if state.peer.is_some_and(|old| old != peer) {
        return Err(ReadinessTransportError::invalid("http_peer_generation_changed"));
    }
    state.peer = Some(peer);
    Ok(())
}
