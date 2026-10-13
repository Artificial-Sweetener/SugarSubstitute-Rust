// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe real empty-owner admission and shutdown without native effects.
use super::*;
use sugarsubstitute_contracts::execution::Moment;
fn owner() -> Result<ProductionActivation> {
    ProductionActivation::new(
        r"E:\application\SugarSubstitute.exe".into(),
        r"E:\application".into(),
        r"D:\captures".into(),
    )
}
#[test]
fn activation_rejects_ambiguous_locations_before_creating_native_owners() {
    assert!(
        ProductionActivation::new(
            "relative.exe".into(),
            r"E:\application".into(),
            r"D:\captures".into(),
        )
        .is_err()
    );
}
#[test]
fn activation_empty_owner_requires_explicit_close_before_finish() -> Result<()> {
    let mut owned: Box<dyn EntryActivation> = Box::new(owner()?);
    assert!(owned.finish().is_err());
    owned.request_close(None)?;
    assert!(owned.poll_close(
        &CompletionBatch { notifications: Vec::new(), remaining: 0 },
        None,
        ShutdownBudget::new(Moment::from_nanos(0))?
    )?);
    owned.finish()?;
    Ok(())
}
#[test]
fn activation_shutdown_keeps_first_cutoffs_across_later_requests() -> Result<()> {
    let mut owned: Box<dyn EntryActivation> = Box::new(owner()?);
    let first = ShutdownBudget::new(Moment::from_nanos(20))?;
    let later = ShutdownBudget::new(Moment::from_nanos(30))?;
    owned.request_close(Some(first))?;
    owned.request_close(Some(later))?;
    assert_eq!(owned.shutdown_budget(), Some(first));
    let earlier = ShutdownBudget::new(Moment::from_nanos(10))?;
    owned.request_close(Some(earlier))?;
    assert_eq!(owned.shutdown_budget(), Some(earlier));
    Ok(())
}
