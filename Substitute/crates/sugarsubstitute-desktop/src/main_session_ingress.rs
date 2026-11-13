// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode exact native session envelopes without acquiring backend, storage or window authority.
use crate::{DesktopError, MainIdentity, SessionBootstrap, SessionObservation};
use serde::Deserialize;
use sugarsubstitute_contracts::{
    desktop_session::{MAX_MESSAGE_BYTES, wire::SessionMessage},
    execution::MemoryLease,
};
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Envelope {
    session_transport: u8,
    webview: String,
    ticket: u64,
    message: SessionMessage,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Header {
    session_transport: u8,
    webview: String,
    ticket: u64,
    #[serde(rename = "message")]
    _message: serde::de::IgnoredAny,
}
pub(crate) struct Received {
    #[cfg(target_os = "windows")]
    pub(crate) ticket: u64,
    pub(crate) observation: Option<SessionObservation>,
}
/// The caller supplies actual native source evidence and the current independently trusted
/// bootstrap. A malformed body may reject its known transport ticket; a foreign source cannot.
pub(crate) fn decode(
    bootstrap: &SessionBootstrap,
    identity: MainIdentity,
    source: &str,
    payload: &str,
    memory: &MemoryLease,
    source_lease: &MemoryLease,
) -> Result<Option<Received>, DesktopError> {
    if !crate::main_navigation::admits_url(bootstrap.origin(), source)
        || payload.len() > crate::main_session_contract::MAX_INGRESS_BYTES
    {
        return Ok(None);
    }
    let claim = match memory.claim((payload.len() as u64) * 4 + 4096) {
        Ok(value) => value,
        Err(cause) => {
            tracing::warn!(operation="session_message_admission",cause=%cause,"Workspace input refused without changing durable state");
            return Ok(None);
        }
    };
    let Ok(header) = serde_json::from_str::<Header>(payload) else { return Ok(None) };
    if header.session_transport != 1
        || header.webview != bootstrap.webview()
        || header.ticket == 0
        || header.ticket > 9_007_199_254_740_991
    {
        return Ok(None);
    }
    let envelope = match serde_json::from_str::<Envelope>(payload) {
        Ok(value) => value,
        Err(_) => {
            return Ok(Some(Received {
                #[cfg(target_os = "windows")]
                ticket: header.ticket,
                observation: None,
            }));
        }
    };
    if envelope.session_transport != header.session_transport
        || envelope.webview != header.webview
        || envelope.ticket != header.ticket
    {
        return Ok(None);
    }
    let message = envelope.message;
    let encoded = serde_json::to_vec(&message)
        .map_err(|cause| DesktopError::new("measure_session_message", cause))?;
    if encoded.len() > MAX_MESSAGE_BYTES || bootstrap.validate(&message).is_err() {
        return Ok(Some(Received {
            #[cfg(target_os = "windows")]
            ticket: header.ticket,
            observation: None,
        }));
    }
    Ok(Some(Received {
        #[cfg(target_os = "windows")]
        ticket: header.ticket,
        observation: Some(SessionObservation::new(identity, message, source_lease.clone(), claim)),
    }))
}
#[cfg(test)]
mod tests;
