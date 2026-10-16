// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise managed timing with real campaign/execution/cleanup owners and external HTTP fixtures.
//! The read-only HTTP fixture grants no managed family authority; Root owns native-start qualification.
use super::*;
use sugarsubstitute_contracts::configuration::LaunchConfiguration;

fn managed_campaign() -> TestResult<ReadinessCampaign> {
    let root = std::env::temp_dir().join("managed-campaign-policy");
    let config = LaunchConfiguration::new(root.clone(), root.join("python.exe"), vec![], 8188)?;
    Ok(ReadinessCampaign::new(
        BackendTarget::Managed(config),
        ConnectionBudget::new(ConnectionBudgetKind::Managed, Moment::from_nanos(0))?,
    )?)
}

#[test]
fn managed_live_admission_requires_current_preparation_and_preserves_attached_timeout() -> TestResult
{
    let mut attached = ReadinessCampaign::new(
        attached_target(8188)?,
        ConnectionBudget::new(ConnectionBudgetKind::Attached, Moment::from_nanos(0))?,
    )?;
    assert!(matches!(
        attached.admit_managed_live(Moment::from_nanos(0)),
        Err(CampaignError::Rejected(CampaignRejection::WrongTarget))
    ));
    assert_eq!(attached.campaign_until(), Some(Moment::from_nanos(30_000_000_000)));
    assert_eq!(
        attached.consume(&empty_batch(), Moment::from_nanos(30_000_000_000))?,
        CampaignStep::Failed
    );
    let mut expired = managed_campaign()?;
    assert!(matches!(
        expired.admit_managed_live(Moment::from_nanos(180_000_000_000)),
        Err(CampaignError::Budget(ProbeBudgetError::Expired))
    ));
    assert_eq!(expired.campaign_until(), Some(Moment::from_nanos(180_000_000_000)));
    let mut live = managed_campaign()?;
    let original = live.budget;
    live.admit_managed_live(Moment::from_nanos(179_000_000_000))?;
    assert_eq!(
        live.consume(&empty_batch(), Moment::from_nanos(600_000_000_000))?,
        CampaignStep::NeedsInput
    );
    assert_eq!(
        live.probe_until(Moment::from_nanos(600_000_000_000))?,
        Moment::from_nanos(602_000_000_000)
    );
    live.admit_managed_live(Moment::from_nanos(600_000_000_000))?;
    assert_eq!(live.budget, original);
    assert_eq!(live.campaign_until(), None);
    assert!(matches!(
        live.probe_until(Moment::from_nanos(599_999_999_999)),
        Err(CampaignError::Rejected(CampaignRejection::ClockRegression))
    ));
    let shutdown = ShutdownBudget::new(Moment::from_nanos(600_000_000_000))?;
    live.cancel(shutdown);
    assert!(matches!(
        live.admit_managed_live(Moment::from_nanos(600_000_000_000)),
        Err(CampaignError::Rejected(CampaignRejection::WrongPhase))
    ));
    assert!(live.settled());
    Ok(())
}

#[test]
fn managed_live_retry_closes_actual_outputs_on_first_fifteen_second_cutoff() -> TestResult {
    let mut h = Harness::new()?;
    h.campaign = managed_campaign()?;
    h.campaign.admit_managed_live(h.now())?;
    h.set_time(200_000_000_000);
    h.submit(Behavior::Unavailable, None, false, None)?;
    assert_eq!(h.completion()?, CampaignStep::Progressed);
    assert!(!h.campaign.settled());
    let original = h.campaign.cleanup_deadline(h.now())?;
    assert_eq!(original, Moment::from_nanos(215_000_000_000));
    assert_eq!(h.campaign.next_deadline(), Some(original));
    h.set_time(201_000_000_000);
    assert_eq!(h.campaign.cleanup_deadline(h.now())?, original);
    let spec = h.spec(Moment::from_nanos(216_000_000_000))?;
    assert!(matches!(
        h.campaign.submit_cleanup(&h.client, &h.cleanup, spec),
        Err(CampaignError::Rejected(CampaignRejection::InvalidSpecification))
    ));
    assert_eq!(h.campaign.cleanup_deadline(h.now())?, original);
    h.submit_close(original)?;
    assert_eq!(h.completion()?, CampaignStep::Progressed);
    assert!(h.campaign.settled());
    assert_eq!(h.campaign.next_deadline(), Some(Moment::from_nanos(201_050_000_000)));
    h.set_time(201_050_000_000);
    assert!(h.campaign.needs_input(h.now())?);
    h.finish()
}

#[test]
fn managed_live_timing_accepts_late_real_http_result_without_granting_managed_authority()
-> TestResult {
    let mut h = Harness::new()?;
    h.campaign = managed_campaign()?;
    h.campaign.admit_managed_live(h.now())?;
    h.set_time(600_000_000_000);
    let input = ReadinessProbeInput::attached(h.lease.clone(), 8188)?;
    let spec = h.spec(Moment::from_nanos(602_000_000_000))?;
    assert!(matches!(
        h.campaign.submit_attempt(&h.client, &h.attempt, spec, input),
        Err(CampaignError::Rejected(CampaignRejection::WrongTarget))
    ));
    // The existing external fixture drives actual campaign work through read-only input.
    // Its proof stays attached and cannot pass Root's managed-resource publication check.
    h.submit(Behavior::Success, None, false, None)?;
    assert_eq!(h.completion()?, CampaignStep::Ready);
    let proof = h.campaign.take_readiness().ok_or("Missing actual HTTP result")?;
    assert_eq!(proof.managed_resource(), None);
    proof.validate_current()?;
    assert_eq!(h.campaign.next_deadline(), None);
    assert!(!h.campaign.settled());
    drop(proof);
    h.finish()
}

#[test]
fn managed_live_cancel_retains_actual_job_and_original_shutdown_cleanup() -> TestResult {
    let mut h = Harness::new()?;
    h.campaign = managed_campaign()?;
    h.campaign.admit_managed_live(h.now())?;
    h.set_time(200_000_000_000);
    let fixture = h.submit(Behavior::Success, Some("begin"), false, None)?;
    fixture.entered()?;
    h.set_time(201_000_000_000);
    let shutdown = ShutdownBudget::new(h.now())?;
    h.campaign.cancel(shutdown);
    h.campaign.cancel(ShutdownBudget::new(Moment::from_nanos(202_000_000_000))?);
    assert!(!h.campaign.settled());
    assert!(!h.campaign.needs_input(h.now())?);
    fixture.release();
    assert_eq!(h.completion()?, CampaignStep::Failed);
    assert_eq!(h.campaign.cleanup_deadline(h.now())?, shutdown.settle_until);
    assert!(h.campaign.take_readiness().is_none());
    h.submit_close(shutdown.settle_until)?;
    assert_eq!(h.completion()?, CampaignStep::Failed);
    assert!(h.campaign.settled());
    assert_eq!(h.campaign.next_deadline(), None);
    h.finish()
}
