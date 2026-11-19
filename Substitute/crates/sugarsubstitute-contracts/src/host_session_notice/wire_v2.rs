// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

// SugarCubes generates these wire shapes from its session-notice-v2.schema.json; the workspace
// compiles that single source directly so the host and the pack cannot drift apart.
include!("../../../../../Sugar/integrations/contracts/SessionNoticeV2Protocol.generated.rs");

use super::{NoticeTone, PROTOCOL, SessionNoticeError, SessionNoticeVersion};

/// Form the generated host-direction value; the caller validates it before serializing.
pub(super) fn encode(
    message: &super::SessionNoticeHostMessage,
    session: &str,
) -> SessionNoticeHostMessage {
    let (protocol, version, session) =
        (PROTOCOL.to_owned(), SessionNoticeVersion::Two.number(), session.to_owned());
    match message {
        super::SessionNoticeHostMessage::Line { notice, tone, text, lasting } => {
            SessionNoticeHostMessage::Line(SessionNoticeLineMessage {
                protocol,
                version,
                session,
                r#type: "line".into(),
                notice: notice.get(),
                tone: match tone {
                    NoticeTone::Info => SessionNoticeTone::Info,
                    NoticeTone::Warn => SessionNoticeTone::Warn,
                    NoticeTone::Error => SessionNoticeTone::Error,
                },
                text: text.clone(),
                lasting: *lasting,
            })
        }
        super::SessionNoticeHostMessage::NotSaved { notice, workspace, inputs } => {
            SessionNoticeHostMessage::NotSaved(SessionNoticeNotSavedMessage {
                protocol,
                version,
                session,
                r#type: "not-saved".into(),
                notice: notice.get(),
                workspace: *workspace,
                inputs: *inputs,
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
        SessionNoticeClientMessage::Watch(watch) => super::SessionNoticeClientMessage::Watch {
            session: session(watch.session)?,
            version: SessionNoticeVersion::Two,
        },
        SessionNoticeClientMessage::Shown(shown) => super::SessionNoticeClientMessage::Shown {
            session: session(shown.session)?,
            notice: super::NoticeNumber::new(shown.notice)?,
        },
    })
}
