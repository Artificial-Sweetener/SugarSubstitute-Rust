// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise finite probe expiry through actual execution, HTTP-slot custody and campaign cleanup.
//! Existing attached HTTP fixtures exercise timing only and grant no managed native authority.
use super::managed::managed_campaign;
use super::*;
use crate::readiness_transport::{ProbeRetryDisposition, campaign_fixture::FixtureHttp};
use std::{sync::Arc, time::Duration};
fn live() -> TestResult<Harness> {
    let mut h = Harness::new()?;
    h.campaign = managed_campaign()?;
    h.campaign.admit_readiness_wait(&h.attempt, h.now())?;
    h.set_time(600_000_000_000);
    Ok(h)
}
fn expire(h: &mut Harness, fail_close: bool) -> TestResult<(Arc<FixtureHttp>, CampaignStep)> {
    let until = h.campaign.probe_until(h.now())?;
    let fixture = h.submit(Behavior::Success, Some("begin"), fail_close, None)?;
    fixture.entered()?;
    assert_eq!(h.campaign.attempt.as_ref().ok_or("Actual probe absent")?.until, until);
    h.set_time(until.as_nanos());
    fixture.release();
    let step = h.completion()?;
    let Some(CampaignError::Transport(cause)) = h.campaign.failure() else {
        return Err("Actual timeout cause absent".into());
    };
    assert_eq!(cause.stage(), "attempt_timed_out");
    assert!(cause.source().is_some());
    assert!(h.campaign.take_readiness().is_none());
    assert!(!h.campaign.settled());
    assert!(h.retained_bytes() > 0);
    assert!(fixture.closed.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_empty());
    Ok((fixture, step))
}
fn cleanup_retry(h: &mut Harness, fixture: &FixtureHttp, delay: Duration) -> TestResult {
    let frozen = h.campaign.cleanup_deadline(h.now())?;
    assert_eq!(frozen, h.now().checked_add(Duration::from_secs(15))?);
    h.set_time(h.now().as_nanos().checked_add(1).ok_or("Fixture clock exhausted")?);
    assert_eq!(h.campaign.cleanup_deadline(h.now())?, frozen);
    assert!(!h.campaign.needs_input(h.now())?);
    let completed_at = h.now();
    h.submit_close(frozen)?;
    assert_eq!(h.completion()?, CampaignStep::Progressed);
    assert!(h.campaign.settled());
    assert_eq!(
        *fixture.closed.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
        [3, 2, 1]
    );
    let next = h.campaign.next_deadline().ok_or("Original backoff absent")?;
    assert_eq!(next, completed_at.checked_add(delay)?);
    h.set_time(next.as_nanos().checked_sub(1).ok_or("Fixture backoff underflow")?);
    assert!(!h.campaign.needs_input(h.now())?);
    h.set_time(next.as_nanos());
    assert!(h.campaign.needs_input(h.now())?);
    Ok(())
}
#[test]
fn managed_live_repeated_actual_probe_timeouts_cleanup_then_reach_real_http_ready() -> TestResult {
    let mut h = live()?;
    for delay in [Duration::from_millis(50), Duration::from_millis(100)] {
        let (fixture, step) = expire(&mut h, false)?;
        if step != CampaignStep::Progressed {
            h.finish()?;
            return Err(
                "Actual finite probe timeout incorrectly ended admitted live managed waiting"
                    .into(),
            );
        }
        assert_eq!(h.campaign.campaign_until(), None);
        cleanup_retry(&mut h, &fixture, delay)?;
        // Keep the first timeout cause retained: the second timeout must replace it as retryable.
        assert!(
            matches!(h.campaign.failure(), Some(CampaignError::Transport(cause)) if cause.stage() == "attempt_timed_out")
        );
    }
    let start = h.now();
    h.submit(Behavior::Success, None, false, None)?;
    assert_eq!(
        h.campaign.attempt.as_ref().ok_or("Fresh actual probe absent")?.until,
        start.checked_add(Duration::from_secs(2))?
    );
    assert_eq!(h.completion()?, CampaignStep::Ready);
    let proof = h.campaign.take_readiness().ok_or("Actual fresh HTTP result absent")?;
    assert_eq!(proof.managed_resource(), None);
    proof.validate_current()?;
    drop(proof);
    h.finish()
}
#[test]
fn finite_probe_timeout_remains_terminal_for_attached_and_unadmitted_managed_campaigns()
-> TestResult {
    for managed in [false, true] {
        let mut h = Harness::new()?;
        if managed {
            h.campaign = managed_campaign()?;
        }
        let original = h.campaign.campaign_until();
        let (_, step) = expire(&mut h, false)?;
        assert_eq!(step, CampaignStep::Failed);
        assert_eq!(h.campaign.campaign_until(), original);
        assert!(!h.campaign.needs_input(h.now())?);
        assert!(h.campaign.take_readiness().is_none());
        h.finish()?;
    }
    Ok(())
}
#[test]
fn managed_probe_timeout_does_not_hide_fatal_http_cleanup() -> TestResult {
    for behavior in [Behavior::Malformed, Behavior::WrongAsset, Behavior::Panicked] {
        let mut h = live()?;
        let (fixture, step) = expire(&mut h, false)?;
        assert_eq!(step, CampaignStep::Progressed);
        cleanup_retry(&mut h, &fixture, Duration::from_millis(50))?;
        h.submit(behavior, None, false, None)?;
        assert_eq!(h.completion()?, CampaignStep::Failed);
        assert!(
            matches!(h.campaign.failure(), Some(CampaignError::Transport(cause)) if cause.retry_disposition() == ProbeRetryDisposition::Terminal)
        );
        assert!(!h.campaign.needs_input(h.now())?);
        h.finish()?;
    }
    let mut h = live()?;
    let (_, step) = expire(&mut h, true)?;
    assert_eq!(step, CampaignStep::Progressed);
    let frozen = h.campaign.cleanup_deadline(h.now())?;
    h.submit_close(frozen)?;
    let error = h.completion().err().ok_or("Actual failed cleanup admitted a successor")?;
    assert!(!h.campaign.needs_input(h.now())?);
    assert_eq!(h.campaign.cleanup_deadline(h.now())?, frozen);
    assert!(h.campaign.take_readiness().is_none());
    assert!(error.source().is_some());
    drop(error);
    h.finish()
}
#[test]
fn late_success_delivery_retires_then_retries_without_publishing_stale_readiness() -> TestResult {
    let mut h = live()?;
    let until = h.campaign.probe_until(h.now())?;
    let fixture = h.submit(Behavior::Success, Some("close"), false, None)?;
    fixture.entered()?;
    fixture.release();
    h.wait_terminal_without_consuming()?;
    h.set_time(until.as_nanos());
    assert_eq!(h.consume_saved()?, CampaignStep::Progressed);
    assert!(h.campaign.take_readiness().is_none());
    assert!(!h.campaign.needs_input(h.now())?);
    let until = h.campaign.cleanup_deadline(h.now())?;
    h.submit_close(until)?;
    assert_eq!(h.completion()?, CampaignStep::Progressed);
    let next = h.campaign.next_deadline().ok_or("Retry backoff absent")?;
    h.set_time(next.as_nanos());
    assert!(h.campaign.needs_input(h.now())?);
    h.submit(Behavior::Success, None, false, None)?;
    assert_eq!(h.completion()?, CampaignStep::Ready);
    assert!(h.campaign.take_readiness().is_some());
    h.finish()
}
#[test]
fn cancellation_after_actual_managed_probe_timeout_prevents_fresh_admission() -> TestResult {
    let mut h = live()?;
    let (_, step) = expire(&mut h, false)?;
    assert_eq!(step, CampaignStep::Progressed);
    let shutdown = ShutdownBudget::new(h.now())?;
    let first_cleanup = h.campaign.cleanup_deadline(h.now())?;
    h.campaign.cancel(shutdown);
    h.campaign.cancel(ShutdownBudget::new(h.now().checked_add(Duration::from_secs(1))?)?);
    assert_eq!(h.campaign.cleanup_deadline(h.now())?, first_cleanup.min(shutdown.settle_until));
    h.submit_close(first_cleanup.min(shutdown.settle_until))?;
    assert_eq!(h.completion()?, CampaignStep::Failed);
    assert!(h.campaign.settled());
    assert!(!h.campaign.needs_input(h.now())?);
    assert_eq!(h.campaign.next_deadline(), None);
    assert!(h.submit(Behavior::Success, None, false, None).is_err());
    h.finish()
}

#[test]
fn native_body_timeout_after_peer_waits_for_checked_cleanup_before_managed_retry() -> TestResult {
    let mut h = live()?;
    let fixture = h.submit(Behavior::NativeTimeout, Some("close"), false, None)?;
    assert_eq!(h.completion()?, CampaignStep::Progressed);
    let Some(CampaignError::Transport(cause)) = h.campaign.failure() else {
        return Err("Native timeout absent".into());
    };
    assert_eq!(cause.stage(), "http_body_read");
    assert_eq!(cause.retry_disposition(), ProbeRetryDisposition::ProbeIntervalElapsed);
    assert_eq!(
        cause
            .source()
            .and_then(|cause| cause.downcast_ref::<windows::core::Error>())
            .map(windows::core::Error::code),
        Some(windows::core::HRESULT::from_win32(12002))
    );
    assert!(h.campaign.take_readiness().is_none());
    assert!(h.retained_bytes() > 0);
    let until = h.campaign.cleanup_deadline(h.now())?;
    h.submit_close(until)?;
    fixture.entered()?;
    assert!(!h.campaign.settled());
    assert!(!h.campaign.needs_input(h.now())?);
    fixture.release();
    assert_eq!(h.completion()?, CampaignStep::Progressed);
    assert_eq!(
        *fixture.closed.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
        [3, 2, 1]
    );
    assert!(h.campaign.settled());
    let next = h.now().checked_add(Duration::from_millis(50))?;
    assert_eq!(h.campaign.next_deadline(), Some(next));
    h.set_time(next.as_nanos());
    h.submit(Behavior::Success, None, false, None)?;
    assert_eq!(h.completion()?, CampaignStep::Ready);
    h.finish()
}
#[test]
fn native_timeout_preserves_attached_budget_and_failed_cleanup_blocks_managed_retry() -> TestResult
{
    let mut attached = Harness::new()?;
    let original = attached.campaign.campaign_until();
    attached.submit(Behavior::NativeTimeout, None, false, None)?;
    assert_eq!(attached.completion()?, CampaignStep::Failed);
    assert_eq!(attached.campaign.campaign_until(), original);
    assert!(!attached.campaign.needs_input(attached.now())?);
    attached.finish()?;
    let mut h = live()?;
    h.submit(Behavior::NativeTimeout, None, true, None)?;
    assert_eq!(h.completion()?, CampaignStep::Progressed);
    let until = h.campaign.cleanup_deadline(h.now())?;
    h.submit_close(until)?;
    let cause = h.completion().err().ok_or("Native timeout hid failed cleanup")?;
    assert!(cause.source().is_some());
    assert!(!h.campaign.needs_input(h.now())?);
    assert!(h.campaign.take_readiness().is_none());
    drop(cause);
    h.finish()
}
#[test]
fn cancellation_before_native_timeout_return_is_terminal_and_retains_cleanup() -> TestResult {
    let mut h = live()?;
    let fixture = h.submit(Behavior::NativeTimeout, Some("get"), false, None)?;
    fixture.entered()?;
    let shutdown = ShutdownBudget::new(h.now())?;
    h.campaign.cancel(shutdown);
    fixture.release();
    assert_eq!(h.completion()?, CampaignStep::Failed);
    let Some(CampaignError::Transport(cause)) = h.campaign.failure() else {
        return Err("Actual cancellation absent".into());
    };
    assert_eq!(cause.stage(), "attempt_cancelled");
    assert_eq!(cause.retry_disposition(), ProbeRetryDisposition::Terminal);
    assert!(
        fixture
            .phases
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .any(|(stage, _)| *stage == "attempt_cancelled")
    );
    assert!(!h.campaign.settled());
    assert!(!h.campaign.needs_input(h.now())?);
    assert!(h.campaign.take_readiness().is_none());
    assert_eq!(h.campaign.cleanup_deadline(h.now())?, shutdown.settle_until);
    h.finish()
}

#[test]
fn original_deadline_precedes_returned_native_timeout_classification() -> TestResult {
    let mut h = live()?;
    let until = h.campaign.probe_until(h.now())?;
    let fixture = h.submit(Behavior::NativeTimeout, Some("get"), false, None)?;
    fixture.entered()?;
    h.set_time(until.as_nanos());
    fixture.release();
    assert_eq!(h.completion()?, CampaignStep::Progressed);
    assert!(
        fixture
            .phases
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .any(|(stage, _)| *stage == "attempt_deadline")
    );
    let Some(CampaignError::Transport(cause)) = h.campaign.failure() else {
        return Err("Execution expiry absent".into());
    };
    assert_eq!(cause.stage(), "attempt_timed_out");
    assert!(h.campaign.take_readiness().is_none());
    assert!(!h.campaign.needs_input(h.now())?);
    assert!(!h.campaign.settled());
    h.finish()
}

#[test]
fn submillisecond_native_budget_retires_managed_probe_but_preserves_attached_cutoff() -> TestResult
{
    for managed_live in [true, false] {
        let mut h = if managed_live { live()? } else { Harness::new()? };
        let original = h.campaign.campaign_until();
        let until = h.campaign.probe_until(h.now())?;
        let fixture = h.submit(Behavior::SubmillisecondBudget, Some("begin"), false, None)?;
        fixture.entered()?;
        h.set_time(until.as_nanos().checked_sub(1).ok_or("Original probe deadline underflow")?);
        fixture.release();
        assert_eq!(
            h.completion()?,
            if managed_live { CampaignStep::Progressed } else { CampaignStep::Failed }
        );
        let Some(CampaignError::Transport(cause)) = h.campaign.failure() else {
            return Err("Actual submillisecond budget outcome absent".into());
        };
        assert_eq!(cause.stage(), "http_submillisecond_deadline");
        assert_eq!(cause.retry_disposition(), ProbeRetryDisposition::ProbeIntervalElapsed);
        assert_eq!(
            cause
                .source()
                .and_then(|cause| cause.downcast_ref::<std::io::Error>())
                .map(std::io::Error::kind),
            Some(std::io::ErrorKind::TimedOut)
        );
        assert_eq!(h.campaign.campaign_until(), original);
        assert!(h.campaign.take_readiness().is_none());
        assert!(!h.campaign.needs_input(h.now())?);
        assert!(!h.campaign.settled());
        assert!(h.retained_bytes() > 0);
        assert!(
            fixture.closed.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_empty()
        );
        if managed_live {
            cleanup_retry(&mut h, &fixture, Duration::from_millis(50))?;
            h.submit(Behavior::Success, None, false, None)?;
            assert_eq!(h.completion()?, CampaignStep::Ready);
        }
        h.finish()?;
    }
    Ok(())
}
