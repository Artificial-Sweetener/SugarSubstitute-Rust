// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

// SugarCubes generates these wire shapes from its release-notes.schema.json; the workspace compiles
// that single source directly so the host and the pack cannot drift apart.
include!("../../../../../Sugar/integrations/contracts/ReleaseNotesProtocol.generated.rs");

use super::{PROTOCOL, ReleaseNotesError, VERSION};

/// Form the generated host-direction value; the caller validates it before serializing.
pub(super) fn encode(
    message: &super::ReleaseNotesHostMessage,
    session: &str,
) -> ReleaseNotesHostMessage {
    let (protocol, version, session) = (PROTOCOL.to_owned(), VERSION, session.to_owned());
    match message {
        super::ReleaseNotesHostMessage::State { revision, product, current_version } => {
            ReleaseNotesHostMessage::State(ReleaseNotesStateMessage {
                protocol,
                version,
                session,
                r#type: "state".into(),
                revision: *revision,
                product: product.clone(),
                current_version: current_version.as_str().to_owned(),
            })
        }
        super::ReleaseNotesHostMessage::Open => {
            ReleaseNotesHostMessage::Open(ReleaseNotesOpenMessage {
                protocol,
                version,
                session,
                r#type: "open".into(),
            })
        }
        super::ReleaseNotesHostMessage::Notes { request, releases, omitted } => {
            ReleaseNotesHostMessage::Notes(ReleaseNotesNotesMessage {
                protocol,
                version,
                session,
                r#type: "notes".into(),
                request: request.as_str().to_owned(),
                releases: releases
                    .iter()
                    .map(|release| ReleaseNotesRelease {
                        version: release.version.as_str().to_owned(),
                        notes: release.notes.clone(),
                    })
                    .collect(),
                omitted: *omitted,
            })
        }
        super::ReleaseNotesHostMessage::Failed { request, reason } => {
            ReleaseNotesHostMessage::Failed(ReleaseNotesFailedMessage {
                protocol,
                version,
                session,
                r#type: "failed".into(),
                request: request.as_str().to_owned(),
                reason: match reason {
                    super::ReleaseNotesFailure::Unavailable => ReleaseNotesFailure::Unavailable,
                    super::ReleaseNotesFailure::NotPublished => ReleaseNotesFailure::NotPublished,
                },
            })
        }
    }
}

/// Translate a validated generated page-direction value into its owned message.
pub(super) fn decode(
    message: ReleaseNotesClientMessage,
) -> Result<super::ReleaseNotesClientMessage, ReleaseNotesError> {
    let session =
        |value: String| crate::panel::PanelSession::new(value).map_err(ReleaseNotesError::Session);
    Ok(match message {
        ReleaseNotesClientMessage::Watch(watch) => {
            super::ReleaseNotesClientMessage::Watch(session(watch.session)?)
        }
        ReleaseNotesClientMessage::Load(load) => super::ReleaseNotesClientMessage::Load {
            session: session(load.session)?,
            request: super::ReleaseNotesRequestName::new(&load.request)?,
            since: super::ReleaseNotesVersion::new(&load.since)?,
        },
    })
}
