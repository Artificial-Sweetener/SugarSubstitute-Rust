// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Translate between the owned updates messages and each released wire version.
//!
//! SugarCubes generates every version's wire shapes from its own schema (`updates.schema.json`
//! for version 1, `updates-v2.schema.json` for version 2); the workspace compiles those sources
//! directly so the host and the pack cannot drift apart. Version 2 adds the product a state
//! describes; version 1 carries everything else unchanged.

macro_rules! updates_wire {
    ($version:literal $(, $product:ident)?) => {
        use crate::host_updates::{self as owned, UpdatesError};

        /// Encode `message` for `session` in this version after the schema's own checks.
        pub(in crate::host_updates) fn encode(
            message: &owned::UpdatesHostMessage,
            session: &str,
        ) -> Result<Vec<u8>, UpdatesError> {
            let value = generated(message, session);
            if !value.valid() {
                return Err(UpdatesError::Invalid);
            }
            serde_json::to_vec(&value).map_err(UpdatesError::Malformed)
        }

        /// Decode a page-direction message of this version.
        pub(in crate::host_updates) fn decode(
            bytes: &[u8],
        ) -> Result<owned::UpdatesClientMessage, UpdatesError> {
            let message: UpdatesClientMessage =
                serde_json::from_slice(bytes).map_err(UpdatesError::Malformed)?;
            if !message.valid() {
                return Err(UpdatesError::Invalid);
            }
            owned_message(message)
        }

        /// Form the generated host-direction value; the caller validates it before serializing.
        fn generated(message: &owned::UpdatesHostMessage, session: &str) -> UpdatesHostMessage {
            let envelope =
                |kind: &str| (owned::PROTOCOL.to_owned(), $version, session.to_owned(), kind.to_owned());
            match message {
                owned::UpdatesHostMessage::State { revision, update } => {
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
                owned::UpdatesHostMessage::Accepted(request) => {
                    let (protocol, version, session, r#type) = envelope("accepted");
                    UpdatesHostMessage::Accepted(UpdatesAcceptedMessage {
                        protocol,
                        version,
                        session,
                        r#type,
                        request: request.as_str().to_owned(),
                    })
                }
                owned::UpdatesHostMessage::Rejected { request, reason } => {
                    let (protocol, version, session, r#type) = envelope("rejected");
                    UpdatesHostMessage::Rejected(UpdatesRejectedMessage {
                        protocol,
                        version,
                        session,
                        r#type,
                        request: request.as_str().to_owned(),
                        reason: match reason {
                            owned::UpdatesRejection::Busy => UpdatesRejection::Busy,
                            owned::UpdatesRejection::NotReady => UpdatesRejection::NotReady,
                            owned::UpdatesRejection::Unavailable => UpdatesRejection::Unavailable,
                        },
                    })
                }
            }
        }

        fn state(value: &owned::UpdatesState) -> UpdatesState {
            UpdatesState {
                $( $product: value.$product.clone(), )?
                current_version: value.current_version.as_str().to_owned(),
                channel: match value.channel {
                    owned::UpdatesChannel::Stable => UpdatesChannel::Stable,
                    owned::UpdatesChannel::Beta => UpdatesChannel::Beta,
                },
                last_successful_check: value
                    .last_successful_check_ms
                    .map(|completed_at| UpdatesCheck { completed_at }),
                status: match &value.status {
                    owned::UpdatesStatus::UpToDate => UpdatesStatus::UpToDate {},
                    owned::UpdatesStatus::Checking => UpdatesStatus::Checking {},
                    owned::UpdatesStatus::Available(version) => {
                        UpdatesStatus::Available { version: version.as_str().to_owned() }
                    }
                    owned::UpdatesStatus::Ready(version) => {
                        UpdatesStatus::Ready { version: version.as_str().to_owned() }
                    }
                    owned::UpdatesStatus::Failed(during) => UpdatesStatus::Failed {
                        during: match during {
                            owned::UpdatesFailure::Check => UpdatesFailure::Check,
                            owned::UpdatesFailure::Update => UpdatesFailure::Update,
                        },
                    },
                },
                report: value.report.as_ref().map(|report| UpdatesReport {
                    version: report.version.as_str().to_owned(),
                    phase: match report.phase {
                        owned::UpdatesPhase::Check => UpdatesPhase::Check,
                        owned::UpdatesPhase::Download => UpdatesPhase::Download,
                        owned::UpdatesPhase::Verify => UpdatesPhase::Verify,
                        owned::UpdatesPhase::Install => UpdatesPhase::Install,
                        owned::UpdatesPhase::Start => UpdatesPhase::Start,
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
                    owned::UpdatesScheduledRestart::None => UpdatesScheduledRestart::None,
                    owned::UpdatesScheduledRestart::AfterWorkflow => UpdatesScheduledRestart::AfterWorkflow,
                },
            }
        }

        /// Translate a validated generated page-direction value into its owned message.
        fn owned_message(
            message: UpdatesClientMessage,
        ) -> Result<owned::UpdatesClientMessage, UpdatesError> {
            let session =
                |value: String| crate::panel::PanelSession::new(value).map_err(UpdatesError::Session);
            Ok(match message {
                UpdatesClientMessage::Watch(watch) => {
                    owned::UpdatesClientMessage::Watch(session(watch.session)?)
                }
                UpdatesClientMessage::Check(check) => owned::UpdatesClientMessage::Check {
                    session: session(check.session)?,
                    request: owned::UpdatesRequestName::new(&check.request)?,
                },
                UpdatesClientMessage::Restart(restart) => owned::UpdatesClientMessage::Restart {
                    session: session(restart.session)?,
                    request: owned::UpdatesRequestName::new(&restart.request)?,
                    when: match restart.when {
                        UpdatesRestartTiming::Now => owned::UpdatesRestartTiming::Now,
                        UpdatesRestartTiming::AfterWorkflow => owned::UpdatesRestartTiming::AfterWorkflow,
                    },
                },
            })
        }
    };
}

pub(super) mod v1 {
    include!("../../../../../Sugar/integrations/contracts/UpdatesProtocol.generated.rs");
    updates_wire!(1);
}

pub(super) mod v2 {
    include!("../../../../../Sugar/integrations/contracts/UpdatesV2Protocol.generated.rs");
    updates_wire!(2, product);
}
