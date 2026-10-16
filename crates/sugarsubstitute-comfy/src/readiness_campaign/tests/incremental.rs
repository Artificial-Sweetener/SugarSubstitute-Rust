// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify the complete large cohort through real bounded jobs and retained HTTP custody.
use super::*;
use std::sync::atomic::Ordering;

#[test]
fn all_1359_assets_qualify_across_timely_steps_with_one_original_claim() -> TestResult {
    let mut harness = Harness::with_asset_count(1359)?;
    let fixture = harness.submit_timed(Behavior::Success, None, false, None, 2_000_000)?;
    let mut steps = 0;
    loop {
        steps += 1;
        let step = harness.completion()?;
        if step == CampaignStep::Ready {
            break;
        }
        assert_eq!(step, CampaignStep::NeedsContinuation);
        assert!(harness.campaign.take_readiness().is_none());
        assert!(!harness.campaign.needs_input(harness.now())?);
        assert!(harness.campaign.needs_continuation(harness.now())?);
        assert!(harness.retained_bytes() >= crate::readiness_transport::READINESS_TRANSPORT_BYTES);
        harness.continue_probe()?;
        assert!(steps < 24);
    }
    let proof = harness.campaign.take_readiness().ok_or("Missing complete cohort proof")?;
    assert_eq!(proof.source().manifest().assets().len(), 1359);
    proof.validate_current()?;
    let requests = fixture.requests.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let assets = requests
        .iter()
        .filter(|path| !matches!(path.as_str(), "/system_stats" | "/extensions"))
        .collect::<Vec<_>>();
    assert_eq!(assets.len(), 1359);
    let unique = assets.iter().copied().collect::<std::collections::BTreeSet<_>>();
    assert_eq!(unique.len(), 1359);
    assert_eq!(requests[requests.len() - 2..], ["/system_stats", "/extensions"]);
    assert_eq!(requests.len(), 1363);
    drop(requests);
    assert!(harness.now().as_nanos() > 2_000_000_000);
    assert_eq!(harness.campaign.budget.until(), Moment::from_nanos(30_000_000_000));
    assert_eq!(steps, 23);
    assert!(
        fixture
            .phases
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .all(|(_, until)| *until <= harness.campaign.budget.until().as_nanos())
    );
    drop(proof);
    harness.finish()
}

#[test]
fn wrong_later_asset_is_terminal_after_checked_partial_success() -> TestResult {
    let mut harness = Harness::with_asset_count(132)?;
    let fixture = harness.submit(Behavior::Success, None, false, None)?;
    assert_eq!(harness.completion()?, CampaignStep::NeedsContinuation);
    fixture.corrupt_asset.store(70, Ordering::Release);
    harness.continue_probe()?;
    assert_eq!(harness.completion()?, CampaignStep::Failed);
    let Some(CampaignError::Transport(cause)) = harness.campaign.failure() else {
        return Err("Missing original hash failure".into());
    };
    assert_eq!(cause.stage(), "served_asset_bytes");
    assert!(harness.campaign.take_readiness().is_none());
    assert!(!harness.campaign.needs_continuation(harness.now())?);
    harness.finish()
}

#[test]
fn timeout_cannot_commit_a_continuation_or_publish_readiness() -> TestResult {
    let mut harness = Harness::with_asset_count(132)?;
    let fixture = harness.submit(Behavior::Success, None, false, None)?;
    assert_eq!(harness.completion()?, CampaignStep::NeedsContinuation);
    let before = fixture.requests.lock().unwrap_or_else(std::sync::PoisonError::into_inner).len();
    fixture.response_nanos.store(2_000_000_000, Ordering::Release);
    harness.continue_probe()?;
    assert_eq!(harness.completion()?, CampaignStep::Failed);
    assert!(harness.campaign.take_readiness().is_none());
    assert!(!harness.campaign.needs_continuation(harness.now())?);
    let requests = fixture.requests.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(&requests[before..], ["/assets/64.bin"]);
    drop(requests);
    harness.finish()
}

#[test]
fn partial_phase_never_renews_original_campaign_cutoff() -> TestResult {
    let mut harness = Harness::with_asset_count(132)?;
    let fixture = harness.submit(Behavior::Success, None, false, None)?;
    assert_eq!(harness.completion()?, CampaignStep::NeedsContinuation);
    let before = fixture.requests.lock().unwrap_or_else(std::sync::PoisonError::into_inner).len();
    harness.set_time(30_000_000_000);
    assert_eq!(harness.campaign.consume(&empty_batch(), harness.now())?, CampaignStep::Failed);
    assert!(harness.continue_probe().is_err());
    assert_eq!(harness.campaign.budget.until(), Moment::from_nanos(30_000_000_000));
    assert_eq!(
        fixture.requests.lock().unwrap_or_else(std::sync::PoisonError::into_inner).len(),
        before
    );
    assert!(harness.campaign.take_readiness().is_none());
    harness.finish()
}

#[test]
fn failed_partial_close_blocks_continuation_and_preserves_original_cause() -> TestResult {
    let mut harness = Harness::with_asset_count(132)?;
    harness.submit(Behavior::Success, None, true, None)?;
    assert_eq!(harness.completion()?, CampaignStep::Failed);
    let Some(CampaignError::Transport(cause)) = harness.campaign.failure() else {
        return Err("Missing original close failure".into());
    };
    assert_eq!(cause.stage(), "fixture_close");
    assert!(harness.retained_bytes() > 0);
    assert!(harness.continue_probe().is_err());
    assert!(harness.campaign.take_readiness().is_none());
    harness.finish()
}

#[test]
fn partial_continuation_cannot_replace_its_original_scope_or_expand_its_phase() -> TestResult {
    let mut harness = Harness::with_asset_count(132)?;
    let fixture = harness.submit(Behavior::Success, None, false, None)?;
    assert_eq!(harness.completion()?, CampaignStep::NeedsContinuation);
    let before = fixture.requests.lock().unwrap_or_else(std::sync::PoisonError::into_inner).len();
    let spec = harness.spec(Moment::from_nanos(2_000_000_001))?;
    assert!(matches!(
        harness.campaign.submit_continuation(&harness.client, &harness.attempt, spec),
        Err(CampaignError::Rejected(CampaignRejection::InvalidSpecification))
    ));
    let spec = harness.spec(Moment::from_nanos(2_000_000_000))?;
    assert!(matches!(
        harness.campaign.submit_continuation(&harness.client, &harness.cleanup, spec),
        Err(CampaignError::Admission(_))
    ));
    assert!(!harness.campaign.needs_continuation(harness.now())?);
    assert!(harness.campaign.take_readiness().is_none());
    assert_eq!(
        fixture.requests.lock().unwrap_or_else(std::sync::PoisonError::into_inner).len(),
        before
    );
    assert!(harness.retained_bytes() > 0);
    harness.finish()
}

#[test]
fn slow_asset_responses_yield_with_original_one_second_retirement_headroom() -> TestResult {
    let mut harness = Harness::with_asset_count(64)?;
    let fixture = harness.submit_timed(Behavior::Success, None, false, None, 30_000_000)?;
    assert_eq!(harness.completion()?, CampaignStep::NeedsContinuation);
    assert_eq!(harness.now(), Moment::from_nanos(1_020_000_000));
    let requests = fixture.requests.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(requests.len(), 34);
    assert_eq!(&requests[..2], ["/system_stats", "/extensions"]);
    drop(requests);
    assert!(harness.campaign.take_readiness().is_none());
    assert_eq!(harness.campaign.budget.until(), Moment::from_nanos(30_000_000_000));
    harness.continue_probe()?;
    assert_eq!(harness.completion()?, CampaignStep::NeedsContinuation);
    assert!(harness.campaign.take_readiness().is_none());
    let before = fixture.requests.lock().unwrap_or_else(std::sync::PoisonError::into_inner).len();
    assert_eq!(before, 66);
    harness.continue_probe()?;
    assert_eq!(harness.completion()?, CampaignStep::Ready);
    let requests = fixture.requests.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(&requests[before..], ["/system_stats", "/extensions"]);
    drop(requests);
    let proof = harness.campaign.take_readiness().ok_or("Missing confirmed yielded cohort")?;
    assert_eq!(proof.source().manifest().assets().len(), 64);
    proof.validate_current()?;
    assert_eq!(harness.now(), Moment::from_nanos(2_040_000_000));
    drop(proof);
    harness.finish()
}

#[test]
fn final_metadata_change_rejects_the_complete_pinned_cohort() -> TestResult {
    let mut harness = Harness::with_asset_count(132)?;
    let fixture = harness.submit(Behavior::Success, None, false, None)?;
    assert_eq!(harness.completion()?, CampaignStep::NeedsContinuation);
    fixture.stale_metadata.store(true, Ordering::Release);
    for _ in 0..2 {
        harness.continue_probe()?;
        assert_eq!(harness.completion()?, CampaignStep::NeedsContinuation);
        assert!(harness.campaign.take_readiness().is_none());
    }
    let before = fixture.requests.lock().unwrap_or_else(std::sync::PoisonError::into_inner).len();
    assert_eq!(before, 134);
    harness.continue_probe()?;
    assert_eq!(harness.completion()?, CampaignStep::Failed);
    let requests = fixture.requests.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(&requests[before..], ["/system_stats"]);
    drop(requests);
    let Some(CampaignError::Transport(cause)) = harness.campaign.failure() else {
        return Err("Missing current metadata failure".into());
    };
    assert_eq!(cause.stage(), "stats_response");
    assert!(harness.campaign.take_readiness().is_none());
    assert!(!harness.campaign.needs_continuation(harness.now())?);
    harness.finish()
}

#[test]
fn single_phase_cohort_confirms_current_metadata_after_all_asset_bodies() -> TestResult {
    let mut harness = Harness::with_asset_count(64)?;
    let fixture = harness.submit(Behavior::Success, Some("get"), false, None)?;
    fixture.entered()?;
    fixture.change_metadata_after_asset.store(true, Ordering::Release);
    fixture.release();
    assert_eq!(harness.completion()?, CampaignStep::Failed);
    let requests = fixture.requests.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(requests.len(), 67);
    assert_eq!(&requests[..2], ["/system_stats", "/extensions"]);
    assert_eq!(requests.last().map(String::as_str), Some("/system_stats"));
    drop(requests);
    let Some(CampaignError::Transport(cause)) = harness.campaign.failure() else {
        return Err("Missing post-asset metadata failure".into());
    };
    assert_eq!(cause.stage(), "stats_response");
    assert!(harness.campaign.take_readiness().is_none());
    harness.finish()
}

#[test]
fn actual_zero_asset_phase_waits_for_original_cutoff_without_resubmission() -> TestResult {
    let mut h = Harness::with_asset_count(132)?;
    h.set_time(28_500_000_000);
    let external = h.submit(Behavior::Success, Some("get"), false, None)?;
    external.entered()?;
    h.set_time(29_000_000_000);
    external.release();
    assert_eq!(h.completion()?, CampaignStep::Waiting);
    let charge = h.retained_bytes();
    assert!(charge > 0);
    for now in [29_000_000_000, 29_500_000_000, 29_999_999_999] {
        h.set_time(now);
        for _ in 0..32 {
            assert_eq!(h.campaign.consume(&empty_batch(), h.now())?, CampaignStep::Waiting);
            assert!(!h.campaign.needs_continuation(h.now())?);
            assert_eq!(h.campaign.next_deadline(), Some(Moment::from_nanos(30_000_000_000)));
            assert!(h.campaign.failure().is_none());
            assert!(h.campaign.take_readiness().is_none());
            assert_eq!(h.retained_bytes(), charge);
        }
    }
    assert_eq!(
        *external.requests.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
        ["/system_stats", "/extensions"]
    );
    h.set_time(30_000_000_000);
    assert_eq!(h.campaign.consume(&empty_batch(), h.now())?, CampaignStep::Failed);
    assert!(matches!(h.campaign.failure(), Some(CampaignError::Budget(ProbeBudgetError::Expired))));
    assert_eq!(h.campaign.budget.until(), Moment::from_nanos(30_000_000_000));
    h.finish()
}

#[test]
fn partial_asset_headroom_has_exact_admission_boundary_and_original_wait() -> TestResult {
    let mut h = Harness::with_asset_count(132)?;
    let external = h.submit(Behavior::Success, None, false, None)?;
    assert_eq!(h.completion()?, CampaignStep::NeedsContinuation);
    let before = external.requests.lock().unwrap_or_else(std::sync::PoisonError::into_inner).len();
    assert_eq!(before, 66);
    h.set_time(28_999_999_999);
    assert!(h.campaign.needs_continuation(h.now())?);
    assert_eq!(h.campaign.consume(&empty_batch(), h.now())?, CampaignStep::NeedsContinuation);
    assert_eq!(h.campaign.next_deadline(), Some(h.now()));
    h.set_time(29_000_000_000);
    assert!(!h.campaign.needs_continuation(h.now())?);
    assert_eq!(h.campaign.consume(&empty_batch(), h.now())?, CampaignStep::Waiting);
    assert_eq!(h.campaign.next_deadline(), Some(Moment::from_nanos(30_000_000_000)));
    assert!(matches!(h.continue_probe(), Err(error)
        if error.downcast_ref::<CampaignError>().is_some_and(|cause|
            matches!(cause, CampaignError::Rejected(CampaignRejection::WrongPhase)))));
    assert_eq!(
        external.requests.lock().unwrap_or_else(std::sync::PoisonError::into_inner).len(),
        before
    );
    assert!(h.campaign.failure().is_none());
    h.set_time(30_000_000_000);
    assert_eq!(h.campaign.consume(&empty_batch(), h.now())?, CampaignStep::Failed);
    h.finish()
}

#[test]
fn final_metadata_only_phase_can_finish_below_asset_headroom() -> TestResult {
    let mut h = Harness::with_asset_count(128)?;
    let external = h.submit(Behavior::Success, None, false, None)?;
    assert_eq!(h.completion()?, CampaignStep::NeedsContinuation);
    h.continue_probe()?;
    assert_eq!(h.completion()?, CampaignStep::NeedsContinuation);
    let before = external.requests.lock().unwrap_or_else(std::sync::PoisonError::into_inner).len();
    assert_eq!(before, 130);
    h.set_time(29_500_000_000);
    assert!(h.campaign.needs_continuation(h.now())?);
    h.continue_probe()?;
    assert_eq!(h.completion()?, CampaignStep::Ready);
    let requests = external.requests.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(&requests[before..], ["/system_stats", "/extensions"]);
    drop(requests);
    let proof = h.campaign.take_readiness().ok_or("Missing actual final confirmation")?;
    assert_eq!(proof.source().manifest().assets().len(), 128);
    proof.validate_current()?;
    assert_eq!(h.campaign.budget.until(), Moment::from_nanos(30_000_000_000));
    drop(proof);
    h.finish()
}

#[test]
fn requested_short_continuation_cannot_admit_zero_asset_work() -> TestResult {
    let mut h = Harness::with_asset_count(132)?;
    h.submit(Behavior::Success, None, false, None)?;
    assert_eq!(h.completion()?, CampaignStep::NeedsContinuation);
    let spec = h.spec(Moment::from_nanos(1_000_000_000))?;
    assert!(matches!(
        h.campaign.submit_continuation(&h.client, &h.attempt, spec),
        Err(CampaignError::Rejected(CampaignRejection::InvalidSpecification))
    ));
    assert!(h.campaign.needs_continuation(h.now())?);
    h.continue_probe()?;
    assert_eq!(h.completion()?, CampaignStep::NeedsContinuation);
    h.finish()
}
