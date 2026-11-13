// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Attribute bounded panel messages to the current native top-document context.

use serde::Deserialize;
use sugarsubstitute_contracts::{
    canvas_native::CanvasNativeRequest,
    canvas_surface::CanvasSurfaceMessage,
    host_updates::UpdatesClientMessage,
    memory::MemoryMessage,
    panel::{PanelMessage, PanelSession},
};

/// One fixed browser binding; its existence grants no host authority.
pub(crate) const BINDING_NAME: &str = "__sugarcubes_substitute_send_v3";
const MAX_EVENT_BYTES: usize = 32768;

/// Private framing version one uses `1:<lane>:<ticket>:<public JSON>` inside the binding string.
/// The public JSON is borrowed intact; delimiters inside it are never interpreted as framing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Lane {
    Ordinary,
    Control,
    Cancellation,
}

impl Lane {
    /// Address the fixed ordinary, control and cancellation storage slots.
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Ordinary => 0,
            Self::Control => 1,
            Self::Cancellation => 2,
        }
    }

    /// Encode the private ASCII lane token.
    pub(crate) const fn token(self) -> &'static str {
        match self {
            Self::Ordinary => "o",
            Self::Control => "c",
            Self::Cancellation => "x",
        }
    }
}

/// A private positive ticket that JavaScript can represent without rounding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Ticket(pub(crate) u64);

/// One native transport action, qualified again before its actual completion grants capacity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(target_os = "windows")]
pub(crate) struct TransportAction {
    pub(crate) lane: Lane,
    pub(crate) ticket: Ticket,
    pub(crate) kind: TransportActionKind,
}

/// Retry only callbacks lost to overload; malformed operations must never replay automatically.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(target_os = "windows")]
pub(crate) enum TransportActionKind {
    Grant,
    RetryableRefusal,
    PermanentRejection,
}

#[cfg(target_os = "windows")]
impl TransportAction {
    /// Use existing private actions: consumed invalid controls/cancels have no ordinary outcome.
    pub(crate) const fn code(self) -> &'static str {
        match self.kind {
            TransportActionKind::Grant => "g",
            TransportActionKind::RetryableRefusal => "r",
            TransportActionKind::PermanentRejection => match self.lane {
                Lane::Ordinary => "r",
                Lane::Control | Lane::Cancellation => "a",
            },
        }
    }
}

/// Own the bounded binding payload while lending its unmodified public JSON to the decoder.
pub(crate) struct Frame {
    pub(crate) lane: Lane,
    pub(crate) ticket: Ticket,
    encoded: String,
    offset: usize,
}

impl Frame {
    /// Borrow exactly the public protocol message following the fixed framing prefix.
    pub(crate) fn payload(&self) -> &str {
        &self.encoded[self.offset..]
    }
}

/// Attribute private framing before consuming an issued ticket or transferring its memory claim.
pub(crate) fn frame(event: &str, current_context: Option<i64>) -> Option<Frame> {
    if event.len() > MAX_EVENT_BYTES {
        return None;
    }
    let event: BindingEvent = serde_json::from_str(event).ok()?;
    if event.name != BINDING_NAME || Some(event.execution_context_id) != current_context {
        return None;
    }
    let mut parts = event.payload.splitn(4, ':');
    if parts.next()? != "1" {
        return None;
    }
    let lane = match parts.next()? {
        "o" => Lane::Ordinary,
        "c" => Lane::Control,
        "x" => Lane::Cancellation,
        _ => return None,
    };
    let digits = parts.next()?;
    if digits.is_empty() || !digits.bytes().all(|value| value.is_ascii_digit()) {
        return None;
    }
    let ticket = digits.parse::<u64>().ok()?;
    if !(1..=9007199254740991).contains(&ticket) {
        return None;
    }
    let offset = event.payload.len() - parts.next()?.len();
    if event.payload.len() - offset > 16384 {
        return None;
    }
    Some(Frame { lane, ticket: Ticket(ticket), encoded: event.payload, offset })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct BindingEvent {
    name: String,
    payload: String,
    execution_context_id: i64,
}

/// The public messages attributed to one current top-document execution context.
#[derive(Debug, PartialEq)]
pub(crate) enum ClientMessage {
    Panel(PanelMessage),
    Surface(CanvasSurfaceMessage),
    Native(CanvasNativeRequest),
    Memory(MemoryMessage),
    Updates(UpdatesClientMessage),
}

impl ClientMessage {
    /// Keep control and cancellation traffic independent of ordinary response ownership.
    pub(crate) const fn lane(&self) -> Lane {
        match self {
            Self::Panel(_) | Self::Memory(_) | Self::Updates(_) => Lane::Control,
            Self::Surface(CanvasSurfaceMessage::Cancel { .. }) => Lane::Cancellation,
            Self::Surface(_) | Self::Native(_) => Lane::Ordinary,
        }
    }

    /// Correlate cancellation and replies without imposing a global public-ID frontier.
    pub(crate) fn request_id(&self) -> Option<u64> {
        match self {
            Self::Panel(_) | Self::Memory(_) | Self::Updates(_) => None,
            Self::Surface(CanvasSurfaceMessage::Request(value)) => Some(value.request_id()),
            Self::Surface(CanvasSurfaceMessage::Discovery(value)) => Some(value.request_id()),
            Self::Surface(CanvasSurfaceMessage::Presentation(value)) => Some(value.request_id()),
            Self::Surface(CanvasSurfaceMessage::Cancel { request_id, .. }) => Some(*request_id),
            Self::Native(value) => Some(value.request_id()),
        }
    }
}

/// Decode only client-direction panel and canvas messages from the current native context.
/// The caller supplies context only after its origin and current document are established,
/// and must retain an admitted allocation claim for this bounded parse and returned message.
#[cfg(test)]
pub(crate) fn decode(
    event: &str,
    current_context: Option<i64>,
    session: &PanelSession,
) -> Option<ClientMessage> {
    if event.len() > MAX_EVENT_BYTES {
        return None;
    }
    let current_context = current_context?;
    let event: BindingEvent = serde_json::from_str(event).ok()?;
    if event.name != BINDING_NAME || event.execution_context_id != current_context {
        return None;
    }
    decode_payload(&event.payload, session)
}

/// Decode the existing public protocol only while its native admission claim remains retained.
pub(crate) fn decode_payload(payload: &str, session: &PanelSession) -> Option<ClientMessage> {
    if let Ok(message) = PanelMessage::decode(payload.as_bytes()) {
        if message.session() == session
            && matches!(
                message,
                PanelMessage::Ready(_)
                    | PanelMessage::Applied(_)
                    | PanelMessage::Pong { .. }
                    | PanelMessage::Rejected { .. }
            )
        {
            return Some(ClientMessage::Panel(message));
        }
        return None;
    }
    if let Ok(message) = CanvasSurfaceMessage::decode(payload.as_bytes()) {
        return (message.session() == session).then_some(ClientMessage::Surface(message));
    }
    if let Ok(message) = MemoryMessage::decode(payload.as_bytes()) {
        return (message.session() == session).then_some(ClientMessage::Memory(message));
    }
    if let Ok(message) = UpdatesClientMessage::decode(payload.as_bytes()) {
        return (message.session() == session).then_some(ClientMessage::Updates(message));
    }
    let request = CanvasNativeRequest::decode(payload.as_bytes()).ok()?;
    (request.session() == session).then_some(ClientMessage::Native(request))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    fn event(name: &str, context: i64, message: &str) -> String {
        serde_json::json!({"name": name, "payload": message, "executionContextId": context})
            .to_string()
    }

    #[test]
    fn private_framing_borrows_json_and_refuses_unknown_versions_tickets_and_contexts()
    -> Result<(), Box<dyn Error>> {
        let body = r#"{"protocol":"sugarcubes.canvas","version":3,"session":"session-12345678","requestId":1,"type":"context.request"}"#;
        let encoded = event(BINDING_NAME, 17, &format!("1:o:9007199254740991:{body}"));
        let decoded = frame(&encoded, Some(17)).ok_or("Missing private frame")?;
        assert_eq!(decoded.lane, Lane::Ordinary);
        assert_eq!(decoded.ticket, Ticket(9007199254740991));
        assert_eq!(decoded.payload(), body);
        assert_eq!(decoded.lane.token(), "o");
        assert_eq!(decoded.lane.index(), 0);
        let session = PanelSession::new("session-12345678".into())?;
        let message =
            decode_payload(decoded.payload(), &session).ok_or("Missing public request")?;
        assert_eq!(message.lane(), Lane::Ordinary);
        assert_eq!(message.request_id(), Some(1));
        assert!(frame(&encoded, Some(18)).is_none());
        assert!(frame(&encoded, None).is_none());
        for prefix in ["0:o:1:", "1:z:1:", "1:o:0:", "1:o:-1:", "1:o:+1:", "1:o:9007199254740992:"]
        {
            assert!(
                frame(&event(BINDING_NAME, 17, &format!("{prefix}{body}")), Some(17)).is_none()
            );
        }
        assert!(frame(&event(BINDING_NAME, 17, body), Some(17)).is_none());
        assert!(
            frame(&event(BINDING_NAME, 17, &format!("1:o:1:{}", "x".repeat(16385))), Some(17))
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn memory_reports_ride_the_control_lane_for_their_own_session_only()
    -> Result<(), Box<dyn Error>> {
        let session = PanelSession::new("session-12345678".into())?;
        let usage = r#"{"protocol":"sugarcubes.memory","version":1,"session":"session-12345678","type":"usage","usedBytes":10,"demandBytes":20,"consumers":[]}"#;
        let message = decode_payload(usage, &session).ok_or("Memory usage was not decoded")?;
        assert!(matches!(
            message,
            ClientMessage::Memory(MemoryMessage::Usage { used_bytes: 10, .. })
        ));
        assert_eq!(message.lane(), Lane::Control);
        assert_eq!(message.request_id(), None);
        let other = PanelSession::new("session-87654321".into())?;
        assert!(decode_payload(usage, &other).is_none());
        let transport_version = usage.replace(r#""version":1"#, r#""version":3"#);
        assert!(decode_payload(&transport_version, &session).is_none());
        Ok(())
    }

    #[test]
    fn update_requests_ride_the_control_lane_for_their_own_session_only()
    -> Result<(), Box<dyn Error>> {
        let session = PanelSession::new("session-12345678".into())?;
        let envelope =
            r#""protocol":"sugarcubes.updates","version":1,"session":"session-12345678""#;
        for body in [
            r#""type":"watch""#,
            r#""type":"check","request":"check-1""#,
            r#""type":"restart","request":"restart-1","when":"after-workflow""#,
        ] {
            let request = format!("{{{envelope},{body}}}");
            let message = decode_payload(&request, &session).ok_or("Update request missing")?;
            assert!(matches!(message, ClientMessage::Updates(_)), "{request}");
            assert_eq!(message.lane(), Lane::Control);
            assert_eq!(message.request_id(), None);
            let other = PanelSession::new("session-87654321".into())?;
            assert!(decode_payload(&request, &other).is_none());
        }
        // Host-direction messages are never accepted from the page.
        let state = format!(r#"{{{envelope},"type":"accepted","request":"check-1"}}"#);
        assert!(decode_payload(&state, &session).is_none());
        Ok(())
    }

    #[test]
    fn only_current_native_context_and_exact_session_can_deliver_a_client_message()
    -> Result<(), Box<dyn Error>> {
        let session = PanelSession::new("session-12345678".into())?;
        let other = PanelSession::new("foreign-12345678".into())?;
        let ready = PanelMessage::Ready(session.clone());
        let encoded = String::from_utf8(ready.encode()?)?;
        let current = event(BINDING_NAME, 17, &encoded);
        assert_eq!(decode(&current, Some(17), &session), Some(ClientMessage::Panel(ready)));
        assert!(decode(&current, None, &session).is_none());
        assert!(decode(&current, Some(18), &session).is_none());
        assert!(decode(&current, Some(17), &other).is_none());
        assert!(decode(&event("foreign", 17, &encoded), Some(17), &session).is_none());
        let host_message = String::from_utf8(PanelMessage::Ping(session.clone()).encode()?)?;
        assert!(decode(&event(BINDING_NAME, 17, &host_message), Some(17), &session).is_none());
        Ok(())
    }

    #[test]
    fn surface_messages_require_the_same_current_context_and_navigation()
    -> Result<(), Box<dyn Error>> {
        let session = PanelSession::new("session-12345678".into())?;
        let other = PanelSession::new("foreign-12345678".into())?;
        let request = serde_json::json!({
            "protocol":"sugarcubes.canvas", "version":3, "session":session.as_str(),
            "requestId":1, "type":"surface.prepare", "revision":1,
            "address": {"sessionId":"canvas:w", "viewId":"view:a", "windowId":"window:a",
                "attachmentId":"attachment:a", "generation":1}
        })
        .to_string();
        let current = event(BINDING_NAME, 17, &request);
        assert!(matches!(
            decode(&current, Some(17), &session),
            Some(ClientMessage::Surface(CanvasSurfaceMessage::Request(_)))
        ));
        assert!(decode(&current, None, &session).is_none());
        assert!(decode(&current, Some(18), &session).is_none());
        assert!(decode(&current, Some(17), &other).is_none());
        assert!(decode(&event("foreign", 17, &request), Some(17), &session).is_none());
        Ok(())
    }

    #[test]
    fn canvas_context_requires_the_current_document_and_panel_session() -> Result<(), Box<dyn Error>>
    {
        let session = PanelSession::new("session-12345678".into())?;
        let other = PanelSession::new("foreign-12345678".into())?;
        let request = serde_json::json!({
            "protocol":"sugarcubes.canvas", "version":3, "session":session.as_str(),
            "requestId":7, "type":"context.request"
        })
        .to_string();
        let current = event(BINDING_NAME, 17, &request);
        assert!(matches!(
            decode(&current, Some(17), &session),
            Some(ClientMessage::Native(CanvasNativeRequest::Context(_)))
        ));
        assert!(decode(&current, None, &session).is_none());
        assert!(decode(&current, Some(18), &session).is_none());
        assert!(decode(&current, Some(17), &other).is_none());
        assert!(decode(&event("foreign", 17, &request), Some(17), &session).is_none());
        Ok(())
    }

    #[test]
    fn every_native_request_family_rides_the_ordinary_lane_for_its_own_session()
    -> Result<(), Box<dyn Error>> {
        let session = PanelSession::new("session-12345678".into())?;
        let other = PanelSession::new("foreign-12345678".into())?;
        let hover = serde_json::json!({"type":"hover", "workflow":"workflow-none",
            "surface":"node:1:image", "mask":null});
        for (kind, command) in [
            ("source.request", serde_json::json!({"type":"watch", "enabled":true})),
            ("geometry.request", serde_json::json!({"type":"context"})),
            ("edit.request", serde_json::json!({"type":"tool", "tool":"brush"})),
            ("workflow.request", hover),
        ] {
            let request = serde_json::json!({
                "protocol":"sugarcubes.canvas", "version":3, "session":session.as_str(),
                "requestId":9, "type":kind, "command":command,
            })
            .to_string();
            let message = decode_payload(&request, &session).ok_or("Native request was inert")?;
            assert!(matches!(&message, ClientMessage::Native(native) if native.request_id() == 9));
            assert_eq!(message.lane(), Lane::Ordinary);
            assert_eq!(message.request_id(), Some(9));
            assert!(decode_payload(&request, &other).is_none());
            let extended = request.replacen("\"requestId\":9", "\"requestId\":9,\"extra\":1", 1);
            assert!(decode_payload(&extended, &session).is_none());
        }
        Ok(())
    }

    #[test]
    fn malformed_duplicate_extended_and_oversized_events_are_inert() -> Result<(), Box<dyn Error>> {
        let session = PanelSession::new("session-12345678".into())?;
        for event in [
            "{}".to_owned(),
            r#"{"name":"__sugarcubes_substitute_send_v3","payload":"{}","executionContextId":17,"executionContextId":17}"#.to_owned(),
            r#"{"name":"__sugarcubes_substitute_send_v3","payload":"{}","executionContextId":17,"other":true}"#.to_owned(),
            " ".repeat(MAX_EVENT_BYTES + 1),
        ] {
            assert!(decode(&event, Some(17), &session).is_none());
        }
        Ok(())
    }
}
