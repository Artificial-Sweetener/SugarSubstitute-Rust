// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve actual execution dispositions and bounded panic causes across HTTP result delivery.
use super::ReadinessTransportError;
use std::{error::Error, fmt};
use sugarsubstitute_contracts::execution::{JobId, JobOutcome, PanicCause, TerminalKind};
#[derive(Debug)]
pub(super) struct ProbePanic(pub(super) PanicCause);
impl fmt::Display for ProbePanic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}
impl Error for ProbePanic {}

/// Retain actual execution disposition and replacement correlation without cause text inference.
#[derive(Debug)]
pub(super) struct ProbeTerminal {
    pub(super) kind: TerminalKind,
    pub(super) replacement: Option<JobId>,
}
impl fmt::Display for ProbeTerminal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Readiness execution outcome: {:?}; replacement: {:?}",
            self.kind, self.replacement
        )
    }
}
impl Error for ProbeTerminal {}

/// Preserve genuine terminal provenance; finite timeout policy belongs to the campaign owner.
pub(super) fn observe(
    outcome: JobOutcome<(), ReadinessTransportError>,
) -> Result<(), ReadinessTransportError> {
    let kind = outcome.kind();
    let (stage, replacement) = match outcome {
        JobOutcome::Success(()) => return Ok(()),
        JobOutcome::Failure(cause) => return Err(cause),
        JobOutcome::Panicked(cause) => {
            return Err(ReadinessTransportError::new("attempt_panicked", ProbePanic(cause)));
        }
        JobOutcome::TimedOut => {
            return Err(ReadinessTransportError::elapsed(
                "attempt_timed_out",
                ProbeTerminal { kind, replacement: None },
            ));
        }
        JobOutcome::Cancelled => ("attempt_cancelled", None),
        JobOutcome::Coalesced { replacement } => ("attempt_coalesced", Some(replacement)),
    };
    Err(ReadinessTransportError::new(stage, ProbeTerminal { kind, replacement }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::readiness_transport::ProbeRetryDisposition;
    #[test]
    fn original_cancellation_and_coalescing_retain_disposition_without_retry()
    -> Result<(), Box<dyn Error>> {
        let replacement = JobId::new(17)?;
        for (outcome, expected, stage, replaced) in [
            (JobOutcome::Cancelled, TerminalKind::Cancelled, "attempt_cancelled", None),
            (
                JobOutcome::Coalesced { replacement },
                TerminalKind::Coalesced,
                "attempt_coalesced",
                Some(replacement),
            ),
        ] {
            let error = observe(outcome).err().ok_or("Actual disposition lost")?;
            assert_eq!(error.stage(), stage);
            assert_eq!(error.retry_disposition(), ProbeRetryDisposition::Terminal);
            let cause = error
                .source()
                .and_then(|cause| cause.downcast_ref::<ProbeTerminal>())
                .ok_or("Typed disposition lost")?;
            assert_eq!(cause.kind, expected);
            assert_eq!(cause.replacement, replaced);
        }
        Ok(())
    }
    #[test]
    fn actual_finite_probe_timeout_retains_typed_terminal_as_context_dependent_expiry()
    -> Result<(), Box<dyn Error>> {
        let error = observe(JobOutcome::TimedOut).err().ok_or("Actual timeout lost")?;
        assert_eq!(error.stage(), "attempt_timed_out");
        assert_eq!(error.retry_disposition(), ProbeRetryDisposition::ProbeIntervalElapsed);
        let original = error
            .source()
            .and_then(|cause| cause.downcast_ref::<ProbeTerminal>())
            .ok_or("Typed timeout provenance lost")?;
        assert_eq!(original.kind, TerminalKind::TimedOut);
        assert_eq!(original.replacement, None);
        Ok(())
    }
}
