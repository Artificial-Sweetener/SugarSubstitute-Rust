// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one actual child-pipe terminal while its registered root exit is classified.
use super::*;
use crate::production::parent_lifecycle::ParentError;
use sugarsubstitute_application::supervision::FailedDesktopFact;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production::entry_activation) enum DesktopFailureDisposition {
    Pending,
    Reportable,
    Fatal,
}
/// The original categorical box and incident interval stay unchanged across observation order.
pub(in crate::production::entry_activation) struct PendingDesktopFailure {
    cause: Box<dyn Error>,
    original: IncidentOperationBudget,
    until: Moment,
}
impl PendingDesktopFailure {
    /// Retain the first original context and the parent's already bound tighter cutoff, if any.
    pub(in crate::production::entry_activation) fn new(
        cause: Box<dyn Error>,
        original: IncidentOperationBudget,
        tighter: Option<Moment>,
    ) -> Self {
        Self {
            cause,
            original,
            until: tighter.map_or(original.until(), |until| until.min(original.until())),
        }
    }
    /// Return the same original interval for genuine collection admission after the exit arrives.
    pub(in crate::production::entry_activation) const fn original(
        &self,
    ) -> IncidentOperationBudget {
        self.original
    }
    /// Return the exclusive classification cutoff without claiming native or family retirement.
    pub(in crate::production::entry_activation) const fn until(&self) -> Moment {
        self.until
    }
    /// Require actual typed origin and exact sealed identities; an absent exit remains pending.
    pub(in crate::production::entry_activation) fn disposition(
        &self,
        fact: Option<FailedDesktopFact>,
        run: RunKey,
        process: ProcessKey,
        root_exited: bool,
        now: Moment,
    ) -> DesktopFailureDisposition {
        if self.cause.downcast_ref::<ParentError>() != Some(&ParentError::DesktopStopped) {
            return DesktopFailureDisposition::Fatal;
        }
        if now >= self.until {
            return DesktopFailureDisposition::Fatal;
        }
        match fact {
            Some(fact) if fact.run() == run && fact.process() == process => {
                DesktopFailureDisposition::Reportable
            }
            Some(_) => DesktopFailureDisposition::Fatal,
            None if root_exited || now >= self.until => DesktopFailureDisposition::Fatal,
            None => DesktopFailureDisposition::Pending,
        }
    }
    /// Move the exact original categorical box; the Parent keeps its actual native completion/cause.
    pub(in crate::production::entry_activation) fn into_cause(self) -> Box<dyn Error> {
        self.cause
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use sugarsubstitute_contracts::lifecycle::*;
    fn fact(instance: u64) -> Result<FailedDesktopFact> {
        let owner = CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        };
        let run = RunKey {
            owner,
            instance: InstanceId::new(instance)?,
            generation: RunGeneration::new(1)?,
        };
        let process = ProcessKey { owner, id: ProcessId::new(instance)? };
        let mut exit =
            sugarsubstitute_application::supervision::DesktopExitTransaction::new(run, process)?;
        exit.observe(DesktopObservation::Exited { process, code: 0 })
            .map_err(|rejected| rejected.reason)?;
        Ok(exit.failed_incident()?)
    }
    #[test]
    fn desktop_failure_both_notice_orders_keep_first_incident_cutoff() -> Result<()> {
        let actual = fact(1)?;
        let original = IncidentOperationBudget::new(Moment::from_nanos(17))?;
        let pending =
            PendingDesktopFailure::new(Box::new(ParentError::DesktopStopped), original, None);
        assert_eq!(
            pending.disposition(
                None,
                actual.run(),
                actual.process(),
                false,
                Moment::from_nanos(18)
            ),
            DesktopFailureDisposition::Pending
        );
        assert_eq!(
            pending.disposition(
                Some(actual),
                actual.run(),
                actual.process(),
                true,
                Moment::from_nanos(19)
            ),
            DesktopFailureDisposition::Reportable
        );
        assert_eq!(pending.original(), original);
        assert_eq!(pending.until(), original.until());
        let fact_first =
            PendingDesktopFailure::new(Box::new(ParentError::DesktopStopped), original, None);
        assert_eq!(
            fact_first.disposition(
                Some(actual),
                actual.run(),
                actual.process(),
                true,
                Moment::from_nanos(20)
            ),
            DesktopFailureDisposition::Reportable
        );
        assert_eq!(fact_first.original(), pending.original());
        Ok(())
    }
    #[test]
    fn desktop_failure_missing_fact_expires_without_settlement_and_respects_tighter_parent()
    -> Result<()> {
        let actual = fact(1)?;
        let original = IncidentOperationBudget::new(Moment::from_nanos(17))?;
        let cutoff = Moment::from_nanos(30);
        let pending = PendingDesktopFailure::new(
            Box::new(ParentError::DesktopStopped),
            original,
            Some(cutoff),
        );
        assert_eq!(
            pending.disposition(
                None,
                actual.run(),
                actual.process(),
                false,
                Moment::from_nanos(29)
            ),
            DesktopFailureDisposition::Pending
        );
        assert_eq!(
            pending.disposition(None, actual.run(), actual.process(), false, cutoff),
            DesktopFailureDisposition::Fatal
        );
        assert_eq!(pending.original(), original);
        assert!(pending.into_cause().downcast_ref::<ParentError>().is_some());
        Ok(())
    }
    #[test]
    fn desktop_failure_fact_at_or_after_first_cutoff_preserves_original_terminal_failure()
    -> Result<()> {
        let actual = fact(1)?;
        let original = IncidentOperationBudget::new(Moment::from_nanos(17))?;
        for now in [original.until(), original.until().checked_add(Duration::from_nanos(1))?] {
            let pending =
                PendingDesktopFailure::new(Box::new(ParentError::DesktopStopped), original, None);
            assert_eq!(
                pending.disposition(Some(actual), actual.run(), actual.process(), true, now),
                DesktopFailureDisposition::Fatal
            );
            assert_eq!(pending.original(), original);
            assert_eq!(
                pending.into_cause().downcast_ref::<ParentError>(),
                Some(&ParentError::DesktopStopped)
            );
        }
        Ok(())
    }
    #[test]
    fn desktop_failure_exact_fact_cannot_hide_foreign_identity_or_root_authority_cause()
    -> Result<()> {
        let actual = fact(1)?;
        let original = IncidentOperationBudget::new(Moment::from_nanos(17))?;
        let pending =
            PendingDesktopFailure::new(Box::new(ParentError::DesktopStopped), original, None);
        assert_eq!(
            pending.disposition(
                Some(fact(2)?),
                actual.run(),
                actual.process(),
                true,
                Moment::from_nanos(18)
            ),
            DesktopFailureDisposition::Fatal
        );
        let authority = PendingDesktopFailure::new(
            Box::new(std::io::Error::other("Original Root authority rejection")),
            original,
            None,
        );
        assert_eq!(
            authority.disposition(
                Some(actual),
                actual.run(),
                actual.process(),
                true,
                Moment::from_nanos(18)
            ),
            DesktopFailureDisposition::Fatal
        );
        Ok(())
    }
}
