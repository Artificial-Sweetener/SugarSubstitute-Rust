// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

// SugarCubes generates these wire shapes from its file-choice.schema.json; the workspace compiles
// that single source directly so the host and the pack cannot drift apart.
include!("../../../../../Sugar/integrations/contracts/FileChoiceProtocol.generated.rs");

use super::{FileChoiceError, PROTOCOL, VERSION};

/// Form the generated host-direction value; the caller validates it before serializing.
pub(super) fn encode(
    message: &super::FileChoiceHostMessage,
    session: &str,
) -> FileChoiceHostMessage {
    let (protocol, version, session) = (PROTOCOL.to_owned(), VERSION, session.to_owned());
    match message {
        super::FileChoiceHostMessage::Chosen { request, name, relative } => {
            FileChoiceHostMessage::Chosen(FileChoiceChosenMessage {
                protocol,
                version,
                session,
                r#type: "chosen".into(),
                request: request.as_str().to_owned(),
                name: name.clone(),
                relative: relative.clone(),
            })
        }
        super::FileChoiceHostMessage::Dismissed { request } => {
            FileChoiceHostMessage::Dismissed(FileChoiceDismissedMessage {
                protocol,
                version,
                session,
                r#type: "dismissed".into(),
                request: request.as_str().to_owned(),
            })
        }
        super::FileChoiceHostMessage::Failed { request, reason } => {
            FileChoiceHostMessage::Failed(FileChoiceFailedMessage {
                protocol,
                version,
                session,
                r#type: "failed".into(),
                request: request.as_str().to_owned(),
                reason: match reason {
                    super::FileChoiceFailure::Busy => FileChoiceFailure::Busy,
                    super::FileChoiceFailure::Unavailable => FileChoiceFailure::Unavailable,
                },
            })
        }
    }
}

/// Translate a validated generated page-direction value into its owned message.
pub(super) fn decode(
    message: FileChoiceClientMessage,
) -> Result<super::FileChoiceClientMessage, FileChoiceError> {
    let session =
        |value: String| crate::panel::PanelSession::new(value).map_err(FileChoiceError::Session);
    let folder = |value: String| {
        if value.is_empty() { Err(FileChoiceError::Invalid) } else { Ok(value) }
    };
    Ok(match message {
        FileChoiceClientMessage::Open(open) => super::FileChoiceClientMessage {
            session: session(open.session)?,
            request: super::FileChoiceRequestName::new(&open.request)?,
            folder: folder(open.folder)?,
            start: open.start,
            extension: super::FileChoiceExtension::new(&open.extension)?,
            purpose: super::FileChoicePurpose::Open,
        },
        FileChoiceClientMessage::Save(save) => super::FileChoiceClientMessage {
            session: session(save.session)?,
            request: super::FileChoiceRequestName::new(&save.request)?,
            folder: folder(save.folder)?,
            start: save.start,
            extension: super::FileChoiceExtension::new(&save.extension)?,
            purpose: super::FileChoicePurpose::Save { name: save.name },
        },
    })
}
