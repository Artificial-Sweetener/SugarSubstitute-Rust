// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

// SugarCubes generates these wire shapes from its backend.schema.json; the workspace compiles
// that single source directly so the host and the pack cannot drift apart.
include!("../../../../../Sugar/integrations/contracts/BackendProtocol.generated.rs");

use super::{BackendError, PROTOCOL, VERSION};

/// Form the generated host-direction value; the caller validates it before serializing.
pub(super) fn encode(message: &super::BackendHostMessage, session: &str) -> BackendStateMessage {
    let super::BackendHostMessage::State { revision, status } = *message;
    BackendStateMessage {
        protocol: PROTOCOL.to_owned(),
        version: VERSION,
        session: session.to_owned(),
        r#type: "state".into(),
        revision,
        status: match status {
            super::BackendStatus::Running => BackendStatus::Running,
            super::BackendStatus::Stopped => BackendStatus::Stopped,
            super::BackendStatus::RetryReady => BackendStatus::RetryReady,
            super::BackendStatus::Retrying => BackendStatus::Retrying,
        },
    }
}

/// Translate a validated generated page-direction value into its owned message.
pub(super) fn decode(
    message: BackendClientMessage,
) -> Result<super::BackendClientMessage, BackendError> {
    let session =
        |value: String| crate::panel::PanelSession::new(value).map_err(BackendError::Session);
    Ok(match message {
        BackendClientMessage::Watch(watch) => {
            super::BackendClientMessage::Watch { session: session(watch.session)? }
        }
        BackendClientMessage::Retry(retry) => super::BackendClientMessage::Retry {
            session: session(retry.session)?,
            revision: retry.revision,
        },
    })
}
