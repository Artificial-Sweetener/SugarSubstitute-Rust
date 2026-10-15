// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Deny known unavailable Restart before invoking the actual incident policy mutation.
use super::*;
/// `None` is an actionable rejection, never acknowledgement or successful restart.
/// Presentation eligibility is checked again at this authoritative Root boundary.
pub(super) fn admit<T>(
    command: ReportCommand,
    availability: ReportRestartAvailability,
    apply: impl FnOnce() -> Result<T>,
) -> Result<Option<T>> {
    if command.action() == ReportAction::DismissAndRequestRestart && !availability.allows_restart()
    {
        return Ok(None);
    }
    apply().map(Some)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unavailable_restart_never_invokes_the_acknowledgement_policy() -> Result<()> {
        use sugarsubstitute_contracts::lifecycle::*;
        let owner = CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        };
        let run =
            RunKey { owner, instance: InstanceId::new(1)?, generation: RunGeneration::new(1)? };
        let binding = ReportBinding::new(
            IncidentKey::new(IncidentSubject::Desktop(run)),
            IncidentRevision::new(1)?,
            ReportSessionId::new(1)?,
        );
        let command = ReportCommand::new(binding, ReportAction::DismissAndRequestRestart);
        let mut calls = 0;
        let rejected = admit(command, ReportRestartAvailability::Unavailable, || {
            calls += 1;
            Ok(())
        })?;
        assert!(rejected.is_none());
        assert_eq!(calls, 0);
        assert_eq!(command.binding(), binding);
        let accepted = admit(command, ReportRestartAvailability::Available, || {
            calls += 1;
            Ok(())
        })?;
        assert!(accepted.is_some());
        assert_eq!(calls, 1);
        Ok(())
    }
    #[test]
    fn unavailable_restart_does_not_disable_other_policy_actions_or_swallow_errors() -> Result<()> {
        use sugarsubstitute_contracts::lifecycle::*;
        let owner = CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        };
        let binding = ReportBinding::new(
            IncidentKey::new(IncidentSubject::Supervisor(owner)),
            IncidentRevision::new(1)?,
            ReportSessionId::new(1)?,
        );
        for action in [
            ReportAction::CopyText,
            ReportAction::Export(ExportSelection::TextOnly),
            ReportAction::Dismiss,
        ] {
            let mut calls = 0;
            assert!(
                admit(
                    ReportCommand::new(binding, action),
                    ReportRestartAvailability::Unavailable,
                    || {
                        calls += 1;
                        Ok(())
                    }
                )?
                .is_some()
            );
            assert_eq!(calls, 1);
        }
        let original = std::io::Error::from_raw_os_error(5);
        let observed = admit::<()>(
            ReportCommand::new(binding, ReportAction::Dismiss),
            ReportRestartAvailability::Unavailable,
            || Err(Box::new(original)),
        )
        .err()
        .ok_or("Original policy error missing")?;
        assert_eq!(
            observed.downcast_ref::<std::io::Error>().and_then(std::io::Error::raw_os_error),
            Some(5)
        );
        Ok(())
    }
}
