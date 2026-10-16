// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Format supplied bounded capture facts honestly; these fixtures create no native capture proof.
use super::*;
#[test]
fn capture_only_reports_localized_unobserved_exit_and_preserves_exact_native_fault_and_subject()
-> Result<(), Box<dyn Error>> {
    let original = summary()?;
    let IncidentSubject::Desktop(run) = original.seed().key().subject() else {
        return Err("Original desktop fixture changed subject".into());
    };
    let dump = DumpId::new([7; 16])?;
    let fault = FailureEvidence::NativeFault { code: 0xc000_0005, address: 0x7010 };
    for subject in [IncidentSubject::Desktop(run), IncidentSubject::Supervisor(run.owner)] {
        let key = IncidentKey::new(subject);
        let seed = IncidentSeed::new(
            key,
            original.seed().revision(),
            original.seed().build().clone(),
            original.seed().created(),
            original.seed().stage(),
        )?;
        let summary = IncidentSummary::new(
            seed,
            original.revision(),
            SummaryFacts::new(
                original.facts().retention(),
                ExitAttribution::CaptureOnly,
                fault,
                CaptureEvidence::Finished(dump),
                PayloadDisposition::Unavailable,
                original.facts().reporting(),
                SymbolOutcome::NotRequested,
            ),
        )?;
        for locale in ["en", "zh-Hans", "ja", "ko", "es", "unknown"] {
            let (lease, _) =
                lease(ReportView::MAX_BYTES as u64 + ReportView::TRANSFER_HEADER_BYTES);
            let view = ReportView::format(
                &summary,
                binding(&summary)?,
                ledger(&summary)?,
                &[],
                locale,
                &lease,
                false,
            )?;
            let labels = messages(locale).report;
            assert_eq!(
                view.text().lines().filter(|line| *line == labels.exit_unobserved).count(),
                1
            );
            assert!(!view.text().contains(&format!("{}:", labels.exit_code)));
            assert!(view.text().contains(&format!("{}: 0xc0000005 @ 0x7010", labels.native_fault)));
            assert!(view.text().contains(&format!("{}: {}", labels.capture, labels.retained)));
            assert_eq!(view.binding().incident(), key);
            assert_eq!(summary.seed().key().subject(), subject);
            assert_eq!(summary.facts().exit(), ExitAttribution::CaptureOnly);
            assert_eq!(summary.facts().failure(), fault);
            assert_eq!(summary.facts().capture(), CaptureEvidence::Finished(dump));
            assert!(view.text().len() <= ReportView::MAX_BYTES);
            assert!(lease.claim(1).is_err());
        }
    }
    Ok(())
}
