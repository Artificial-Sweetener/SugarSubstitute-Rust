// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

// SugarCubes generates these wire shapes from its updates.schema.json; the workspace compiles that
// single source directly so the host and the pack cannot drift apart.
include!("../../../../../Sugar/integrations/contracts/UpdatesProtocol.generated.rs");

use super::{PROTOCOL, VERSION, UpdatesError};

/// Form the generated host-direction value; the caller validates it before serializing.
pub(super) fn encode(message: &super::UpdatesHostMessage, session: &str) -> UpdatesHostMessage {
    let envelope = |kind: &str| (PROTOCOL.to_owned(), VERSION, session.to_owned(), kind.to_owned());
    match message {
        super::UpdatesHostMessage::State { revision, update } => {
            let (protocol, version, session, r#type) = envelope("state");
            UpdatesHostMessage::State(UpdatesStateMessage {
                protocol,
                version,
                session,
                r#type,
                revision: *revision,
                update: state(update),
            })
        }
        super::UpdatesHostMessage::Accepted(request) => {
            let (protocol, version, session, r#type) = envelope("accepted");
            UpdatesHostMessage::Accepted(UpdatesAcceptedMessage {
                protocol,
                version,
                session,
                r#type,
                request: request.as_str().to_owned(),
            })
        }
        super::UpdatesHostMessage::Rejected { request, reason } => {
            let (protocol, version, session, r#type) = envelope("rejected");
            UpdatesHostMessage::Rejected(UpdatesRejectedMessage {
                protocol,
                version,
                session,
                r#type,
                request: request.as_str().to_owned(),
                reason: match reason {
                    super::UpdatesRejection::Busy => UpdatesRejection::Busy,
                    super::UpdatesRejection::NotReady => UpdatesRejection::NotReady,
                    super::UpdatesRejection::Unavailable => UpdatesRejection::Unavailable,
                },
            })
        }
    }
}

fn state(value: &super::UpdatesState) -> UpdatesState {
    UpdatesState {
        current_version: value.current_version.as_str().to_owned(),
        channel: match value.channel {
            super::UpdatesChannel::Stable => UpdatesChannel::Stable,
            super::UpdatesChannel::Beta => UpdatesChannel::Beta,
        },
        last_successful_check: value
            .last_successful_check_ms
            .map(|completed_at| UpdatesCheck { completed_at }),
        status: match &value.status {
            super::UpdatesStatus::UpToDate => UpdatesStatus::UpToDate {},
            super::UpdatesStatus::Checking => UpdatesStatus::Checking {},
            super::UpdatesStatus::Available(version) => {
                UpdatesStatus::Available { version: version.as_str().to_owned() }
            }
            super::UpdatesStatus::Ready(version) => {
                UpdatesStatus::Ready { version: version.as_str().to_owned() }
            }
            super::UpdatesStatus::Failed(during) => UpdatesStatus::Failed {
                during: match during {
                    super::UpdatesFailure::Check => UpdatesFailure::Check,
                    super::UpdatesFailure::Update => UpdatesFailure::Update,
                },
            },
        },
        report: value.report.as_ref().map(|report| UpdatesReport {
            version: report.version.as_str().to_owned(),
            phase: match report.phase {
                super::UpdatesPhase::Check => UpdatesPhase::Check,
                super::UpdatesPhase::Download => UpdatesPhase::Download,
                super::UpdatesPhase::Verify => UpdatesPhase::Verify,
                super::UpdatesPhase::Install => UpdatesPhase::Install,
                super::UpdatesPhase::Start => UpdatesPhase::Start,
            },
            causes: report
                .causes
                .iter()
                .map(|message| UpdatesCause { message: message.clone() })
                .collect(),
        }),
        previous_version_running: value.previous_version_running,
        workflow_running: value.workflow_running,
        scheduled_restart: match value.scheduled_restart {
            super::UpdatesScheduledRestart::None => UpdatesScheduledRestart::None,
            super::UpdatesScheduledRestart::AfterWorkflow => UpdatesScheduledRestart::AfterWorkflow,
        },
    }
}

/// Translate a validated generated page-direction value into its owned message.
pub(super) fn decode(message: UpdatesClientMessage) -> Result<super::UpdatesClientMessage, UpdatesError> {
    let session = |value: String| {
        crate::panel::PanelSession::new(value).map_err(UpdatesError::Session)
    };
    Ok(match message {
        UpdatesClientMessage::Watch(watch) => super::UpdatesClientMessage::Watch(session(watch.session)?),
        UpdatesClientMessage::Check(check) => super::UpdatesClientMessage::Check {
            session: session(check.session)?,
            request: super::UpdatesRequestName::new(&check.request)?,
        },
        UpdatesClientMessage::Restart(restart) => super::UpdatesClientMessage::Restart {
            session: session(restart.session)?,
            request: super::UpdatesRequestName::new(&restart.request)?,
            when: match restart.when {
                UpdatesRestartTiming::Now => super::UpdatesRestartTiming::Now,
                UpdatesRestartTiming::AfterWorkflow => super::UpdatesRestartTiming::AfterWorkflow,
            },
        },
    })
}
