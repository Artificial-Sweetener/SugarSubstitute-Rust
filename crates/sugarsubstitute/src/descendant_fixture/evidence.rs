// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject root-exit-as-settlement and stale response while retaining actual ordered facts.
use super::*;
#[derive(Default)]
pub(super) struct Evidence {
    pub(super) started: bool,
    pub(super) ready: bool,
    pub(super) root_exited: bool,
    pub(super) root_job: Option<JobId>,
    pub(super) member_job: Option<JobId>,
    pub(super) retained: usize,
    pub(super) challenge: bool,
    pub(super) response: bool,
    pub(super) member_success: bool,
    pub(super) grace: bool,
    pub(super) forced: bool,
    pub(super) settled: bool,
    pub(super) retired: bool,
}
impl Evidence {
    pub(super) fn observe_member(
        &mut self,
        exit: sugarsubstitute_execution::windows::ProcessExit,
        correlation: u64,
        notice: Option<TerminalKind>,
        closing: bool,
    ) -> Result<()> {
        // Closing drains actual ownership even when the original proof was rejected.
        // The headless owner retains that first error, and no success fact is added.
        if closing {
            return Ok(());
        }
        if exit.correlation.get() != correlation
            || exit.process_id == 0
            || exit.creation_time_100ns == 0
            || exit.exit_code != 1
        {
            return Err(
                "Actual descendant member exit identity/status differs from force expectation"
                    .into(),
            );
        }
        if let Some(kind) = notice {
            self.member_terminal(kind)?;
        }
        Ok(())
    }
    pub(super) fn root_exit(&mut self, code: u32) -> Result<()> {
        if !self.started || self.root_exited || code != 0 {
            return Err("Descendant campaign requires one actual natural root exit0".into());
        }
        self.root_exited = true;
        Ok(())
    }
    pub(super) fn can_challenge(&self) -> bool {
        self.root_exited
            && self.ready
            && self.retained == 2
            && self.member_job.is_some()
            && !self.member_success
            && !self.challenge
    }
    pub(super) fn bind_member(&mut self, id: JobId) -> Result<()> {
        if self.member_job.is_some() || self.root_job == Some(id) || !self.root_exited {
            return Err("Descendant member identity is not a new post-root-exit watch".into());
        }
        self.member_job = Some(id);
        Ok(())
    }
    pub(super) fn responded(&mut self) -> Result<()> {
        if !self.root_exited || !self.challenge || self.response || self.member_success {
            return Err("Descendant Response lacks a fresh post-exit Challenge".into());
        }
        self.response = true;
        Ok(())
    }
    pub(super) fn member_terminal(&mut self, kind: TerminalKind) -> Result<()> {
        if !self.response || !self.grace || kind != TerminalKind::Success {
            return Err(
                "Descendant member ended without required survival and original grace proof".into(),
            );
        }
        self.member_success = true;
        Ok(())
    }
    pub(super) fn settlement(&mut self) -> Result<()> {
        if !self.member_success
            || !self.forced
            || !self.root_exited
            || !self.response
            || self.settled
        {
            return Err(
                "Root exit or incomplete child evidence cannot establish family settlement".into(),
            );
        }
        self.settled = true;
        Ok(())
    }
    pub(super) fn complete(&self) -> bool {
        self.started
            && self.ready
            && self.root_exited
            && self.retained == 2
            && self.challenge
            && self.response
            && self.member_success
            && self.grace
            && self.forced
            && self.settled
            && self.retired
    }
    pub(super) fn can_retire_family(&self, actually_settled: bool, closing: bool) -> bool {
        // A rejected proof still retires actually settled ownership during cleanup.
        // The original headless failure remains authoritative; receipt stays incomplete.
        actually_settled && (closing || self.settled)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn root_exit_cannot_establish_family_settlement() -> Result<()> {
        let mut evidence = Evidence { started: true, ..Evidence::default() };
        evidence.root_exit(0)?;
        assert!(!evidence.complete());
        assert!(evidence.settlement().is_err());
        Ok(())
    }
    #[test]
    fn response_before_post_exit_challenge_is_rejected() {
        let mut evidence = Evidence { ready: true, root_exited: true, ..Evidence::default() };
        assert!(evidence.responded().is_err());
        assert!(!evidence.response);
    }
    #[test]
    fn rejected_qualification_still_retires_actual_family_during_owned_shutdown() {
        let evidence = Evidence::default();
        assert!(!evidence.can_retire_family(false, true));
        assert!(!evidence.can_retire_family(true, false));
        assert!(evidence.can_retire_family(true, true));
        assert!(!evidence.complete());
    }
    #[test]
    fn cached_unexpected_exit_does_not_prevent_closing_from_retiring_actual_settlement()
    -> Result<()> {
        let mut evidence = Evidence::default();
        let exit = sugarsubstitute_execution::windows::ProcessExit {
            correlation: NativeCorrelation::new(1)?,
            process_id: 123,
            creation_time_100ns: 456,
            exit_code: 0,
        };
        assert!(evidence.observe_member(exit, 1, Some(TerminalKind::Success), false).is_err());
        evidence.observe_member(exit, 1, Some(TerminalKind::Success), true)?;
        assert!(evidence.can_retire_family(true, true));
        assert!(!evidence.member_success && !evidence.complete());
        Ok(())
    }
    #[test]
    fn member_watch_and_original_grace_are_required_before_actual_settlement() -> Result<()> {
        let mut evidence = Evidence {
            started: true,
            ready: true,
            root_exited: true,
            retained: 2,
            root_job: Some(JobId::new(2)?),
            ..Evidence::default()
        };
        assert!(!evidence.can_challenge());
        assert!(evidence.bind_member(JobId::new(2)?).is_err());
        evidence.bind_member(JobId::new(4)?)?;
        assert!(evidence.can_challenge());
        evidence.challenge = true;
        evidence.responded()?;
        assert!(evidence.member_terminal(TerminalKind::Success).is_err());
        evidence.grace = true;
        assert!(evidence.member_terminal(TerminalKind::Cancelled).is_err());
        evidence.member_terminal(TerminalKind::Success)?;
        assert!(evidence.settlement().is_err());
        evidence.forced = true;
        evidence.settlement()?;
        assert!(!evidence.complete());
        evidence.retired = true;
        assert!(evidence.complete());
        Ok(())
    }
}
