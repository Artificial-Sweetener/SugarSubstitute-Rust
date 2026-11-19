// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

// SugarCubes generates these wire shapes from its update-report.schema.json; the workspace
// compiles that single source directly so the host and the pack cannot drift apart.
include!("../../../../../Sugar/integrations/contracts/UpdateReportProtocol.generated.rs");

use super::{PROTOCOL, UpdateReportError, VERSION};

/// Form the generated host-direction value; the caller validates it before serializing.
pub(super) fn encode(
    message: &super::UpdateReportHostMessage,
    session: &str,
) -> UpdateReportStateMessage {
    let super::UpdateReportHostMessage::State { revision, available } = *message;
    UpdateReportStateMessage {
        protocol: PROTOCOL.to_owned(),
        version: VERSION,
        session: session.to_owned(),
        r#type: "state".into(),
        revision,
        available,
    }
}

/// Translate a validated generated page-direction value into its owned message.
pub(super) fn decode(
    message: UpdateReportClientMessage,
) -> Result<super::UpdateReportClientMessage, UpdateReportError> {
    let session =
        |value: String| crate::panel::PanelSession::new(value).map_err(UpdateReportError::Session);
    Ok(match message {
        UpdateReportClientMessage::Watch(watch) => {
            super::UpdateReportClientMessage::Watch { session: session(watch.session)? }
        }
        UpdateReportClientMessage::Open(open) => super::UpdateReportClientMessage::Open {
            session: session(open.session)?,
            revision: open.revision,
        },
    })
}
