// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Attribute bounded panel messages to the current native top-document context.

use serde::Deserialize;
use sugarsubstitute_contracts::{
    canvas_context::CanvasContextRequest,
    canvas_surface::CanvasSurfaceMessage,
    panel::{PanelMessage, PanelSession},
};

/// One fixed browser binding; its existence grants no host authority.
pub(crate) const BINDING_NAME: &str = "__sugarcubes_substitute_send_v3";
const MAX_EVENT_BYTES: usize = 32768;

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
    Context(CanvasContextRequest),
}

/// Decode only client-direction panel and canvas messages from the current native context.
/// The caller supplies context only after its origin and current document are established,
/// and must retain an admitted allocation claim for this bounded parse and returned message.
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
    if let Ok(message) = PanelMessage::decode(event.payload.as_bytes()) {
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
    if let Ok(message) = CanvasSurfaceMessage::decode(event.payload.as_bytes()) {
        return (message.session() == session).then_some(ClientMessage::Surface(message));
    }
    let request = CanvasContextRequest::decode(event.payload.as_bytes()).ok()?;
    (request.session() == session).then_some(ClientMessage::Context(request))
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
        assert!(matches!(decode(&current, Some(17), &session), Some(ClientMessage::Context(_))));
        assert!(decode(&current, None, &session).is_none());
        assert!(decode(&current, Some(18), &session).is_none());
        assert!(decode(&current, Some(17), &other).is_none());
        assert!(decode(&event("foreign", 17, &request), Some(17), &session).is_none());
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
