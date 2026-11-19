// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

// SugarCubes generates these wire shapes from its session-notice.schema.json; the workspace
// compiles that single source directly so the host and the pack cannot drift apart.
include!("../../../../../Sugar/integrations/contracts/SessionNoticeProtocol.generated.rs");

use super::{PROTOCOL, SessionNoticeError, SessionNoticeVersion};

/// Form the generated host-direction value; the caller validates it before serializing.
///
/// # Errors
/// Rejects a line, which version 1 cannot carry.
pub(super) fn encode(
    message: &super::SessionNoticeHostMessage,
    session: &str,
) -> Result<SessionNoticeHostMessage, SessionNoticeError> {
    let (protocol, version, session) =
        (PROTOCOL.to_owned(), SessionNoticeVersion::One.number(), session.to_owned());
    Ok(match *message {
        super::SessionNoticeHostMessage::Line { .. } => return Err(SessionNoticeError::Invalid),
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
    })
}

/// Translate a validated generated page-direction value into its owned message.
pub(super) fn decode(
    message: SessionNoticeClientMessage,
) -> Result<super::SessionNoticeClientMessage, SessionNoticeError> {
    let session =
        |value: String| crate::panel::PanelSession::new(value).map_err(SessionNoticeError::Session);
    Ok(match message {
        SessionNoticeClientMessage::Watch(watch) => super::SessionNoticeClientMessage::Watch {
            session: session(watch.session)?,
            version: SessionNoticeVersion::One,
        },
        SessionNoticeClientMessage::Shown(shown) => super::SessionNoticeClientMessage::Shown {
            session: session(shown.session)?,
            notice: super::NoticeNumber::new(shown.notice)?,
        },
    })
}
