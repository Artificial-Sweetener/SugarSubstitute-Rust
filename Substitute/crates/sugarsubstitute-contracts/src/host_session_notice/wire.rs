// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

// SugarCubes generates these wire shapes from its session-notice.schema.json; the workspace
// compiles that single source directly so the host and the pack cannot drift apart.
include!("../../../../../Sugar/integrations/contracts/SessionNoticeProtocol.generated.rs");

use super::{PROTOCOL, SessionNoticeError, VERSION};

/// Form the generated host-direction value; the caller validates it before serializing.
pub(super) fn encode(
    message: &super::SessionNoticeHostMessage,
    session: &str,
) -> SessionNoticeHostMessage {
    let (protocol, version, session) = (PROTOCOL.to_owned(), VERSION, session.to_owned());
    match *message {
        super::SessionNoticeHostMessage::NotSaved { notice, workspace, inputs } => {
            SessionNoticeHostMessage::NotSaved(SessionNoticeNotSavedMessage {
                protocol,
                version,
                session,
                r#type: "not-saved".into(),
                notice: notice.get(),
                workspace,
                inputs,
            })
        }
        super::SessionNoticeHostMessage::None => {
            SessionNoticeHostMessage::None(SessionNoticeNoneMessage {
                protocol,
                version,
                session,
                r#type: "none".into(),
            })
        }
    }
}

/// Translate a validated generated page-direction value into its owned message.
pub(super) fn decode(
    message: SessionNoticeClientMessage,
) -> Result<super::SessionNoticeClientMessage, SessionNoticeError> {
    let session =
        |value: String| crate::panel::PanelSession::new(value).map_err(SessionNoticeError::Session);
    Ok(match message {
        SessionNoticeClientMessage::Watch(watch) => {
            super::SessionNoticeClientMessage::Watch(session(watch.session)?)
        }
        SessionNoticeClientMessage::Shown(shown) => super::SessionNoticeClientMessage::Shown {
            session: session(shown.session)?,
            notice: super::NoticeNumber::new(shown.notice)?,
        },
    })
}
