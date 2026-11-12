// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject aliases, live loans and missing genuine receipts under original central funding.
use super::*;
use crate::production::incident_host::{CONTROLLER_BYTES, tests::admitted_value};
#[test]
fn recycling_alias_denial_keeps_exact_original_claim_and_state_after_join()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    admitted_value(CONTROLLER_BYTES, |context| {
        let mut funding = Some(ReporterFunding::new(&context)?);
        let mut source = ReporterSource::new(&mut funding)?;
        assert_eq!(source.close(), Err(ReporterSourceIssue::Busy));
        let pointer = {
            let mut loan = source.loan()?;
            std::ptr::from_ref(loan.state()?) as usize
        };
        let alias = source.alias();
        assert!(matches!(source.recycling_loan(), Err(ReporterSourceIssue::Busy)));
        let remaining = context.memory().claim(
            CONTROLLER_BYTES - crate::production::reporter_role::parent::REPORTER_METADATA_BYTES,
        )?;
        assert!(context.memory().claim(1).is_err());
        drop(remaining);
        drop(alias);
        let mut loan = source.recycling_loan()?;
        assert_eq!(std::ptr::from_ref(loan.state()?) as usize, pointer);
        assert!(loan.state()?.funding.is_some());
        assert!(loan.state()?.parent.is_none());
        drop(loan);
        Ok(source)
    })
}
#[test]
fn recycling_live_loan_and_missing_receipt_never_withdraw_original_funding()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    admitted_value(CONTROLLER_BYTES, |context| {
        let mut funding = Some(ReporterFunding::new(&context)?);
        let mut source = ReporterSource::new(&mut funding)?;
        assert!(matches!(source.recycling_loan(), Err(ReporterSourceIssue::Identity)));
        assert_eq!(source.close(), Err(ReporterSourceIssue::Busy));
        let alias = source.alias();
        let loan = alias.loan()?;
        assert!(matches!(source.recycling_loan(), Err(ReporterSourceIssue::Busy)));
        drop(loan);
        drop(alias);
        let mut receipt = None;
        assert_eq!(source.finish_recycled(&mut receipt), Err(ReporterSourceIssue::Busy));
        let mut loan = source.loan()?;
        assert!(loan.state()?.closing);
        assert!(loan.state()?.funding.is_some());
        drop(loan);
        Ok(source)
    })
}
#[test]
fn recycling_closed_missing_native_owner_cannot_install_another_session()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    admitted_value(CONTROLLER_BYTES, |context| {
        let mut funding = Some(ReporterFunding::new(&context)?);
        let source = ReporterSource::new(&mut funding)?;
        assert_eq!(source.close(), Err(ReporterSourceIssue::Busy));
        let key = DiagnosticKey::new(
            sugarsubstitute_contracts::lifecycle::CoordinatorKey {
                namespace: sugarsubstitute_contracts::lifecycle::NamespaceId::new([1; 32]),
                epoch: sugarsubstitute_contracts::lifecycle::CoordinatorEpoch::new([2; 16])?,
            },
            DiagnosticRole::Reporter,
            DiagnosticSessionId::new(1)?,
        );
        let mut session = None;
        assert_eq!(source.install_owner(&mut session, key), Err(ReporterSourceIssue::Busy));
        assert!(source.loan()?.state()?.funding.is_some());
        Ok(source)
    })
}

#[test]
fn recycling_equal_owner_facts_do_not_match_distinct_actual_source_allocations()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    admitted_value(CONTROLLER_BYTES, |context| {
        let mut funding = Some(ReporterFunding::new(&context)?);
        let first = ReporterSource::new(&mut funding)?;
        let mut other = Some(ReporterFunding::new(&context)?);
        let second = ReporterSource::new(&mut other)?;
        let alias = first.alias();
        assert_eq!(first.shared.owner, second.shared.owner);
        assert!(first.same_allocation(&alias));
        assert!(!first.same_allocation(&second));
        assert!(first.idle_funded());
        assert_eq!(first.close(), Err(ReporterSourceIssue::Busy));
        assert!(!first.idle_funded());
        drop(alias);
        drop(second);
        Ok(first)
    })
}

#[path = "concurrent_tests.rs"]
mod concurrent_tests;
