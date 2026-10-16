// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Expose each retained original report boundary before the native owner selects future work.
use super::*;
impl IncidentReporting {
    /// Clear the retained edge only after the activation owns its immediate native wake.
    /// Blocked admission and in-flight jobs produce no additional continuation.
    pub(in crate::production::entry_activation) fn take_continuation(&mut self) -> bool {
        self.continuation.take().is_some()
    }

    /// Enumerate original nanosecond cutoffs without hiding relay retry behind an older owner.
    /// Due work still owes its progress turn; this iterator never renews a budget or admits work.
    pub(in crate::production::entry_activation) fn deadlines(
        &self,
    ) -> impl Iterator<Item = Moment> + '_ {
        self.stopping
            .into_iter()
            .flat_map(|budget| [budget.graceful_until, budget.settle_until])
            .chain(
                self.store
                    .as_ref()
                    .map(|work| work.cleanup_until.unwrap_or(work.scope.original().until())),
            )
            .chain(self.export_work.as_ref().and_then(|work| work.until()))
            .chain(self.queue_until)
            .chain(self.relay_until)
            .chain(self.relay.as_ref().and_then(ReporterRelay::deadline))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    use std::time::Instant;
    use sugarsubstitute_execution::ExecutionOwner;
    struct FixedClock(AtomicU64);
    impl Clock for FixedClock {
        fn now(&self) -> Moment {
            Moment::from_nanos(self.0.load(Ordering::Acquire))
        }
        fn sleep_until(&self, _: Moment) -> WaitFuture {
            Box::pin(std::future::pending())
        }
    }
    struct Wake;
    impl CompletionWake for Wake {
        fn wake(&self) -> std::result::Result<(), ExecutionError> {
            Ok(())
        }
    }
    fn moment(seconds: u64) -> Moment {
        Moment::from_nanos(seconds * 1_000_000_000)
    }
    #[test]
    fn report_deadlines_retain_old_queue_original_relay_and_both_close_boundaries() -> Result<()> {
        let mut receiving = IncidentReporting::new();
        let queue = Moment::from_nanos(5);
        let relay = Moment::from_nanos(45_000_000_000);
        let closing = ShutdownBudget::new(Moment::from_nanos(60_000_000_000))?;
        receiving.queue_until = Some(queue);
        receiving.relay_until = Some(relay);
        receiving.stopping = Some(closing);
        assert_eq!(
            receiving.deadlines().collect::<Vec<_>>(),
            vec![closing.graceful_until, closing.settle_until, queue, relay]
        );
        assert_eq!(receiving.queue_until, Some(queue));
        assert_eq!(receiving.relay_until, Some(relay));
        assert_eq!(receiving.stopping, Some(closing));
        assert!(receiving.relay.is_none());
        assert!(receiving.failure().is_none());
        Ok(())
    }
    #[test]
    fn activity_arming_old_report_queue_cannot_hide_original_pending_relay_cutoff() -> Result<()> {
        let clock = Arc::new(FixedClock(AtomicU64::new(moment(20).as_nanos())));
        let executor = ExecutionOwner::new(clock.clone(), Arc::new(Wake))?;
        let client = executor.client();
        let mut activation = ProductionActivation::new(
            r"E:\unopened\sugarsubstitute.exe".into(),
            r"E:\unopened".into(),
            r"D:\unopened".into(),
        )?;
        activation.incidents.reporting.queue_until = Some(moment(19));
        activation.incidents.reporting.relay_until = Some(moment(45));
        let anchor = Instant::now();
        activation.arm_next_wake(&client, anchor)?;
        assert_eq!(activation.next_wake, anchor.checked_add(Duration::from_secs(25)));
        assert_eq!(activation.incidents.reporting.queue_until, Some(moment(19)));
        assert_eq!(activation.incidents.reporting.relay_until, Some(moment(45)));
        clock.0.store(moment(45).as_nanos(), Ordering::Release);
        activation.arm_next_wake(&client, anchor)?;
        assert_eq!(activation.next_wake, None);
        assert_eq!(activation.incidents.reporting.relay_until, Some(moment(45)));
        assert!(activation.incidents.reporting.failure().is_none());
        Ok(())
    }
    #[test]
    fn queued_report_continuation_survives_clock_rejection_and_coalesces_into_one_native_turn()
    -> Result<()> {
        let clock = Arc::new(FixedClock(AtomicU64::new(moment(20).as_nanos())));
        let executor = ExecutionOwner::new(clock.clone(), Arc::new(Wake))?;
        let client = executor.client();
        let mut activation = ProductionActivation::new(
            r"E:\unopened\sugarsubstitute.exe".into(),
            r"E:\unopened".into(),
            r"D:\unopened".into(),
        )?;
        activation.incidents.reporting.relay_until = Some(moment(45));
        activation.incidents.reporting.continuation = Some(ReportContinuation::ConsumeEffect);
        activation.incidents.reporting.continuation = Some(ReportContinuation::AdvanceStore);
        let anchor = Instant::now();
        activation.arm_next_wake(&client, anchor)?;
        assert_eq!(activation.next_wake, Some(anchor));
        assert!(activation.incidents.reporting.continuation.is_none());
        activation.arm_next_wake(&client, anchor)?;
        assert_eq!(activation.next_wake, anchor.checked_add(Duration::from_secs(25)));
        assert_eq!(activation.incidents.reporting.relay_until, Some(moment(45)));
        activation.incidents.reporting.continuation = Some(ReportContinuation::ConsumeEffect);
        clock.0.store(moment(19).as_nanos(), Ordering::Release);
        assert!(activation.arm_next_wake(&client, anchor).is_err());
        assert!(matches!(
            activation.incidents.reporting.continuation,
            Some(ReportContinuation::ConsumeEffect)
        ));
        assert_eq!(activation.next_wake, None);
        clock.0.store(moment(20).as_nanos(), Ordering::Release);
        assert!(activation.arm_next_wake(&client, anchor).is_err());
        assert!(matches!(
            activation.incidents.reporting.continuation,
            Some(ReportContinuation::ConsumeEffect)
        ));
        Ok(())
    }
}
