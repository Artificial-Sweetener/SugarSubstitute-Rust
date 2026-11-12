// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep localized degraded coverage explicit inside the same bounded owned report credit.
use super::*;
#[test]
fn degraded_notice_is_optional_localized_and_uses_the_existing_exact_credit()
-> Result<(), Box<dyn Error>> {
    let summary = summary()?;
    let bytes = ReportView::MAX_BYTES as u64 + ReportView::TRANSFER_HEADER_BYTES;
    for locale in ["en", "zh-Hans", "ja", "ko", "es", "unknown"] {
        let (lease, released) = lease(bytes);
        let ordinary = ReportView::format(
            &summary,
            binding(&summary)?,
            ledger(&summary)?,
            &[],
            locale,
            &lease,
            false,
        )?;
        let notice = messages(locale).report.recovery_capture_degraded;
        assert!(!ordinary.text().contains(notice));
        let expected = ordinary.text().to_owned();
        drop(ordinary);
        let degraded = ReportView::format(
            &summary,
            binding(&summary)?,
            ledger(&summary)?,
            &[],
            locale,
            &lease,
            true,
        )?;
        assert_eq!(degraded.text().matches(notice).count(), 1);
        assert_eq!(degraded.text().replace(&format!("{notice}\n"), ""), expected);
        assert!(degraded.text().len() <= ReportView::MAX_BYTES);
        assert!(!degraded.text().contains(['\0', '\r']));
        assert!(degraded.text().ends_with('\n'));
        assert!(lease.claim(1).is_err());
        drop(lease);
        assert_eq!(released.load(Ordering::SeqCst), 0);
        drop(degraded);
        assert_eq!(released.load(Ordering::SeqCst), bytes);
    }
    Ok(())
}
#[test]
fn degraded_coverage_cannot_bypass_binding_or_existing_allocation_bound()
-> Result<(), Box<dyn Error>> {
    let summary = summary()?;
    let bytes = ReportView::MAX_BYTES as u64 + ReportView::TRANSFER_HEADER_BYTES;
    let (insufficient, _) = lease(bytes - 1);
    for root_capture_degraded in [false, true] {
        assert!(matches!(
            ReportView::format(
                &summary,
                binding(&summary)?,
                ledger(&summary)?,
                &[],
                "en",
                &insufficient,
                root_capture_degraded
            ),
            Err(ReportViewError::Credit(_))
        ));
        let untouched = insufficient.claim(bytes - 1)?;
        drop(untouched);
    }
    let (lease, _) = lease(bytes);
    let wrong = ReportBinding::new(
        summary.seed().key(),
        IncidentRevision::new(3)?,
        ReportSessionId::new(1)?,
    );
    assert!(matches!(
        ReportView::format(&summary, wrong, ledger(&summary)?, &[], "en", &lease, true),
        Err(ReportViewError::Identity)
    ));
    let untouched = lease.claim(bytes)?;
    drop(untouched);
    Ok(())
}
