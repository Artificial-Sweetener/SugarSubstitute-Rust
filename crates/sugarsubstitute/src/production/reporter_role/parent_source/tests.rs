// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify original controller funding, nonblocking loan exclusivity and unwind restoration.
use super::*;
use crate::production::incident_host::tests::admitted_value;
#[test]
fn original_funding_transfers_once_and_last_alias_keeps_controller_charge_through_join()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    admitted_value(crate::production::incident_host::CONTROLLER_BYTES, |context| {
        let mut funding = Some(ReporterFunding::new(&context)?);
        let source = ReporterSource::new(&mut funding)?;
        assert!(funding.is_none());
        assert!(matches!(ReporterSource::new(&mut funding), Err(ReporterSourceIssue::Busy)));
        let alias = source.alias();
        drop(source);
        assert_eq!(alias.shared.owner, context.owner_id());
        let remaining = context.memory().claim(
            crate::production::incident_host::CONTROLLER_BYTES
                - super::super::parent::REPORTER_METADATA_BYTES,
        )?;
        assert!(context.memory().claim(1).is_err());
        drop(remaining);
        Ok(alias)
    })
}
#[test]
fn occupied_native_loan_returns_promptly_and_unwind_restores_exact_box_and_custody()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    admitted_value(crate::production::incident_host::CONTROLLER_BYTES, |context| {
        let mut funding = Some(ReporterFunding::new(&context)?);
        let source = ReporterSource::new(&mut funding)?;
        let pointer = {
            let mut loan = source.loan()?;
            std::ptr::from_ref(loan.state()?) as usize
        };
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _loan = source.loan().ok();
            assert!(source.shared.state.try_lock().is_ok());
            assert!(matches!(source.loan(), Err(ReporterSourceIssue::Busy)));
            std::panic::resume_unwind(Box::new(991_u64));
        }));
        assert_eq!(
            panic.as_ref().err().and_then(|cause| cause.downcast_ref::<u64>()),
            Some(&991_u64)
        );
        let after = {
            let mut loan = source.loan()?;
            std::ptr::from_ref(loan.state()?) as usize
        };
        assert_eq!(pointer, after);
        assert!(source.loan()?.state()?.funding.is_some());
        Ok(source)
    })
}
#[test]
fn wrong_core_reservation_rejects_before_withdrawing_original_source()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    admitted_value(crate::production::incident_host::CONTROLLER_BYTES, |context| {
        let mut funding = Some(ReporterFunding::new(&context)?);
        let source = ReporterSource::new(&mut funding)?;
        let counter = CounterDeadline::new(1, 1)?;
        assert_eq!(
            source.process(&context, || Ok(context.now()), |_| Ok(counter)),
            Err(ReporterSourceIssue::Identity)
        );
        assert!(source.loan()?.state()?.funding.is_some());
        Ok(source)
    })
}
#[test]
fn retiring_projection_requires_explicit_close_and_genuine_parent_before_any_settlement()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    admitted_value(crate::production::incident_host::CONTROLLER_BYTES, |context| {
        let mut funding = Some(ReporterFunding::new(&context)?);
        let source = ReporterSource::new(&mut funding)?;
        assert!(matches!(source.poll_retiring(), Err(ReporterSourceIssue::Identity)));
        assert!(matches!(source.close(), Err(ReporterSourceIssue::Busy)));
        assert!(matches!(source.poll_retiring(), Err(ReporterSourceIssue::Busy)));
        let mut loan = source.loan()?;
        assert!(loan.state()?.funding.is_some());
        drop(loan);
        Ok(source)
    })
}
