// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep authenticated startup failure and attempt cleanup separate from process shutdown.
#[path = "attempt/reply.rs"]
mod reply;
use super::*;
use std::{fmt, time::Duration};
use sugarsubstitute_contracts::startup::{StartupFailure, StartupFailureCode};
use sugarsubstitute_supervision::lifecycle_protocol::{FrameCodec, WireMessage, WirePayload};

/// The original authenticated disposition, retained without rendering wire diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AcquisitionFailure {
    Failed(LifecycleFailureCode),
    Rejected(LifecycleRejection),
}
impl fmt::Display for AcquisitionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Authenticated connection disposition: {self:?}")
    }
}
impl Error for AcquisitionFailure {}
impl AcquisitionFailure {
    pub(super) const fn classification(self) -> StartupFailureCode {
        match self {
            Self::Failed(LifecycleFailureCode::Deadline) => StartupFailureCode::TimedOut,
            Self::Rejected(_) => StartupFailureCode::ConnectionPreparation,
            _ => StartupFailureCode::ConnectionUnavailable,
        }
    }
}

pub(super) struct AttemptCleanup {
    original: Moment,
    pending: Option<(WireHeader, bool)>,
    acknowledged: bool,
    settled: bool,
    failures: [Option<Box<dyn Error>>; 2],
    omitted_failures: u64,
}
impl AttemptCleanup {
    /// Original cleanup cutoff, retained independently of failure/timer presentation.
    pub(super) fn original_deadline(&self) -> Moment {
        self.original
    }
    pub(super) fn deadline(&self) -> Option<Moment> {
        if self.settled || self.failures.iter().any(Option::is_some) {
            None
        } else {
            Some(self.original)
        }
    }
}
impl ProductionChild {
    /// Consume the genuine final reply before releasing its passive transport loan.
    /// The application owns the unchanged typed cause through cleanup and presentation.
    pub(super) fn acquire_failed(
        &mut self,
        header: WireHeader,
        cause: AcquisitionFailure,
    ) -> Result<()> {
        if self.events.iter().all(Option::is_some) {
            return Err("Startup failure event capacity exhausted".into());
        }
        self.bootstrap.channel_mut().consume_reply(header)?;
        self.acquire = None;
        self.failed = true;
        self.grant = None;
        self.push_event(StartupEvent::new(
            self.identity.launch,
            StartupObservation::Failed(StartupFailure::new(
                cause.classification(),
                Box::new(cause),
            )),
        ))
    }
    /// Close only this attempt's finite main/output work; capture and channel stay live.
    pub(super) fn begin_attempt_cleanup(&mut self) -> Result<()> {
        if self.attempt.is_some() {
            return Ok(());
        }
        let original = self.client()?.now()?.checked_add(Duration::from_secs(15))?;
        self.main.close();
        self.output.close();
        if self.main.profile().is_some() && !self.shutdown.main_retired {
            let slot = self
                .commands
                .iter_mut()
                .find(|slot| slot.is_none())
                .ok_or("Main retirement command slots full")?;
            *slot = Some(DesktopStartupCommand::RetireMain {
                identity: self.identity,
                reason: if self.frontend_timed_out {
                    sugarsubstitute_desktop::MainRetirementReason::FrontendTimeout
                } else {
                    sugarsubstitute_desktop::MainRetirementReason::AttemptCleanup
                },
            });
        }
        self.attempt = Some(AttemptCleanup {
            original,
            pending: None,
            acknowledged: false,
            settled: false,
            failures: [None, None],
            omitted_failures: 0,
        });
        self.advance_attempt()
    }
    /// Observe each actual finite owner before requesting the matching backend settlement.
    pub(super) fn advance_attempt(&mut self) -> Result<()> {
        let Some(attempt) = &self.attempt else { return Ok(()) };
        if attempt.settled || attempt.failures.iter().any(Option::is_some) {
            return Ok(());
        }
        if self.client()?.now()? >= attempt.original {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Original attempt cleanup expired",
            )
            .into());
        }
        if self.main.pending() || self.output.pending() {
            return Ok(());
        }
        self.main.release_frame()?;
        self.output.release_frame()?;
        if let Some(header) = self.grant_header
            && !self.main.request_consumed
        {
            self.bootstrap.channel_mut().consume_reply(header)?;
            self.main.request_consumed = true;
            self.grant = None;
        }
        if self.grant_header.is_some()
            && self.main.profile().is_none()
            && !self.shutdown.main_retired
            && !self.shutdown.main_absence_pending
        {
            self.push_command(DesktopStartupCommand::ConfirmMainAbsent(self.identity))?;
            self.shutdown.main_absence_pending = true;
            return Ok(());
        }
        if self.shutdown.main_absence_pending
            || ((self.main.profile().is_some() || self.shutdown.main_retired)
                && (!self.shutdown.main_retired || !self.shutdown.main_retirement_ack))
        {
            return Ok(());
        }
        if self.attempt.as_ref().is_some_and(|a| a.pending.is_none()) {
            let original = self.attempt.as_ref().ok_or("Attempt cleanup absent")?.original;
            let header = self.bootstrap.channel_mut().prepare_request(original)?;
            self.attempt.as_mut().ok_or("Attempt cleanup absent")?.pending = Some((header, false));
        }
        let original = self.attempt.as_ref().ok_or("Attempt cleanup absent")?.original;
        let (header, admitted) = self
            .attempt
            .as_ref()
            .and_then(|a| a.pending)
            .ok_or("Attempt cleanup request absent")?;
        if !admitted {
            let acquisition = self
                .acquisition_header
                .and_then(|h| h.request)
                .ok_or("Original acquisition identity absent")?;
            if self.bootstrap.channel_mut().try_send(
                &WireMessage {
                    header,
                    payload: WirePayload::SettleAttempt { identity: self.identity, acquisition },
                },
                original,
            )? {
                self.attempt.as_mut().ok_or("Attempt cleanup absent")?.pending =
                    Some((header, true));
            }
            return Ok(());
        }
        if self.bootstrap.channel_mut().received_header()? != Some(header) {
            return Ok(());
        }
        self.failed_frame = self.bootstrap.channel_mut().take_frame();
        let decoded = FrameCodec::decode(
            self.failed_frame.as_ref().ok_or("Actual settlement reply absent")?.as_bytes(),
        )?;
        match reply::classify(header, &decoded)? {
            reply::SettlementReply::Acknowledged => {}
            reply::SettlementReply::Failed(cause) => {
                self.bootstrap.channel_mut().consume_reply(header)?;
                self.failed_frame = None;
                return self.retain_attempt_failure(Box::new(cause));
            }
        }
        if self.acquire.is_some() {
            return Err("Original acquisition final reply remains unsettled".into());
        }
        if self.events.iter().all(Option::is_some) {
            return Err("Settlement event slots full".into());
        }
        self.bootstrap.channel_mut().consume_reply(header)?;
        self.failed_frame = None;
        let attempt = self.attempt.as_mut().ok_or("Attempt cleanup absent")?;
        attempt.acknowledged = true;
        attempt.settled = true;
        self.lease = None;
        self.push_event(StartupEvent::new(
            self.identity.launch,
            StartupObservation::ConnectionSettled,
        ))
    }
    /// Preserve actual cleanup failure without turning it into native window retirement.
    pub(super) fn retain_attempt_failure(&mut self, cause: Box<dyn Error>) -> Result<()> {
        let attempt = self.attempt.as_mut().ok_or("Attempt failure has no original owner")?;
        if let Some(slot) = attempt.failures.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(cause);
            tracing::warn!(operation="startup_attempt_cleanup",launch=?self.identity.launch,
                "Original cleanup failure retained; retry remains unavailable");
        } else {
            attempt.omitted_failures = attempt.omitted_failures.saturating_add(1);
        }
        Ok(())
    }
    /// Inspect retained failure presentation only; this cannot establish settlement.
    pub(in crate::production) fn cleanup_failed(&self) -> bool {
        self.attempt.as_ref().is_some_and(|attempt| attempt.failures.iter().any(Option::is_some))
    }
    /// Reject foreign or unsettled custody before the application replaces its old failure.
    /// This reads the same actual retained transaction; it grants no physical retirement.
    pub(in crate::production) fn ensure_retry_ready(&self, identity: MainIdentity) -> Result<()> {
        if identity.run != self.run
            || identity.launch.get() <= self.identity.launch.get()
            || self.attempt.as_ref().is_none_or(|a| !a.acknowledged || !a.settled)
            || self.main.pending()
            || self.output.pending()
        {
            return Err("Retry requires the exact original attempt settlement".into());
        }
        if self.failure.is_some() {
            return Err("Unresolved original child infrastructure cause prevents retry".into());
        }
        Ok(())
    }
    /// Replace only a settled acquisition attempt without replacing capture or channel.
    pub(in crate::production) fn retry_attempt(
        &mut self,
        identity: MainIdentity,
        prepared: &mut PreparedChildStartup,
    ) -> Result<()> {
        self.ensure_retry_ready(identity)?;
        let mut input = self.main.take_reusable_input()?;
        if let Err(cause) = prepared.restore_main_input(&mut input) {
            self.main.restore_reusable_input(&mut input)?;
            return Err(cause);
        }
        self.failed = false;
        self.identity = identity;
        self.connection_budget = None;
        self.frontend_budget = None;
        self.frontend_timed_out = false;
        self.acquire = None;
        self.acquisition_header = None;
        self.attempt = None;
        self.grant_header = None;
        self.shutdown.main_retired = false;
        self.shutdown.main_retirement_ack = false;
        self.shutdown.main_absence_pending = false;
        self.main = main::MainPreparation::new(self.main.settings);
        self.output = output::ChildOutput::new();
        self.snapshot = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expired_frontend_routes_existing_cleanup_without_renewing_or_erasing_failure() -> Result<()>
    {
        use super::super::connection::ActivityRoute;
        let frontend = FrontendBudget::new(Moment::from_nanos(0))?;
        let original = frontend.until().checked_add(Duration::from_secs(15))?;
        let cleanup = AttemptCleanup {
            original,
            pending: None,
            acknowledged: false,
            settled: false,
            failures: [
                Some(Box::new(std::io::Error::other("actual initiating boundary failure"))),
                None,
            ],
            omitted_failures: 0,
        };
        let phase = Some(sugarsubstitute_application::startup::StartupPhase::WebviewLoading);
        let now = frontend.until();
        assert_eq!(
            ActivityRoute::select(false, Some(&cleanup), phase, Some(frontend), now)?,
            ActivityRoute::AttemptCleanup
        );
        assert_eq!(
            ActivityRoute::select(false, None, phase, Some(frontend), now)
                .err()
                .map(|cause| cause.kind()),
            Some(std::io::ErrorKind::TimedOut)
        );
        assert_eq!(
            ActivityRoute::select(true, Some(&cleanup), phase, Some(frontend), now)?,
            ActivityRoute::Shutdown
        );
        assert_eq!(cleanup.original_deadline(), original);
        assert!(cleanup.failures[0].as_ref().is_some_and(|cause| cause.is::<std::io::Error>()));
        assert!(cleanup.pending.is_none() && !cleanup.acknowledged && !cleanup.settled);
        assert_eq!(frontend.until(), now);
        Ok(())
    }
    #[test]
    fn authenticated_disposition_retains_typed_failure_and_rejection_classifications() {
        assert_eq!(
            AcquisitionFailure::Failed(LifecycleFailureCode::Preparation).classification(),
            StartupFailureCode::ConnectionUnavailable
        );
        assert_eq!(
            AcquisitionFailure::Failed(LifecycleFailureCode::Deadline).classification(),
            StartupFailureCode::TimedOut
        );
        assert_eq!(
            AcquisitionFailure::Rejected(LifecycleRejection::Overloaded).classification(),
            StartupFailureCode::ConnectionPreparation
        );
        let failure = StartupFailure::new(
            StartupFailureCode::ConnectionUnavailable,
            Box::new(AcquisitionFailure::Failed(LifecycleFailureCode::Cleanup)),
        );
        assert_eq!(
            failure.source().and_then(|cause| cause.downcast_ref::<AcquisitionFailure>()),
            Some(&AcquisitionFailure::Failed(LifecycleFailureCode::Cleanup))
        );
    }
    #[test]
    fn failed_cleanup_keeps_original_cause_and_disables_expired_attempt_timer_without_settlement() {
        let original = Moment::from_nanos(123);
        let mut attempt = AttemptCleanup {
            original,
            pending: None,
            acknowledged: false,
            settled: false,
            failures: [None, None],
            omitted_failures: 0,
        };
        assert_eq!(attempt.deadline(), Some(original));
        attempt.failures[0] =
            Some(Box::new(std::io::Error::other("actual cleanup boundary failure")));
        assert_eq!(attempt.deadline(), None);
        assert_eq!(attempt.original, original);
        assert_eq!(attempt.original_deadline(), original);
        assert!(!attempt.acknowledged && !attempt.settled);
        assert!(attempt.failures[0].as_ref().is_some_and(|cause| cause.is::<std::io::Error>()));
    }
}
