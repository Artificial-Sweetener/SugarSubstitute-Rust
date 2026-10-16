// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Match the original kernel-authenticated Reporter action and issued request before effects.
use super::*;
use sugarsubstitute_crash::diagnostic_client::{DiagnosticCommand, DiagnosticIncident};
/// Require the exact actual same-session original frame for presentation and dismissal.
#[expect(clippy::result_large_err, reason = "Keep funded ReportingOperation cause inline")]
pub(super) fn validate(input: &ReportingInput) -> Result<(), ReportingCause> {
    let Some(expected) = expected_command(&input.work, input.authentication.is_some())? else {
        return Ok(());
    };
    let ReportingAuthentication { received, issued } =
        input.authentication.as_ref().ok_or(ReportingCause::Identity)?;
    let frame = received.decode().map_err(ReportingCause::Wire)?;
    let binding = input.work.binding();
    if received.key().role() != DiagnosticRole::Reporter
        || received.key().owner() != input.operation.owner()
        || frame.key != received.key()
        || frame.request != issued.request()
        || frame.context.incident
            != Some(DiagnosticIncident { key: binding.incident(), revision: binding.revision() })
        || frame.command != expected
    {
        return Err(ReportingCause::Identity);
    }
    Ok(())
}
/// Decide only the required closed command shape; a true flag alone grants no authentication.
#[expect(clippy::result_large_err, reason = "Keep funded ReportingOperation cause inline")]
pub(super) fn expected_command(
    work: &ReportingWork,
    has_frame: bool,
) -> Result<Option<DiagnosticCommand<'static>>, ReportingCause> {
    let expected = match work {
        ReportingWork::View { .. } | ReportingWork::Symbols { .. } => return Ok(None),
        ReportingWork::Attempt { attempt: ReportAttempt::Failed(_), .. } if !has_frame => {
            return Ok(None);
        }
        ReportingWork::Attempt { binding, attempt: ReportAttempt::Presented } => {
            DiagnosticCommand::Presented(binding.session())
        }
        ReportingWork::Attempt { binding, attempt: ReportAttempt::Failed(failure) } => {
            DiagnosticCommand::ReporterFailed { session: binding.session(), failure: *failure }
        }
        ReportingWork::Acknowledge { command, .. } => {
            if !matches!(
                command.action(),
                ReportAction::Dismiss | ReportAction::DismissAndRequestRestart
            ) {
                return Err(ReportingCause::Identity);
            }
            DiagnosticCommand::Action {
                session: command.binding().session(),
                action: command.action(),
            }
        }
    };
    if !has_frame {
        return Err(ReportingCause::Identity);
    }
    Ok(Some(expected))
}
