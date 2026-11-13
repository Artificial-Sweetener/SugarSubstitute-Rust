// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify inert cross-crate transfer and native options before native effects.
use std::{
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use sugarsubstitute_contracts::{
    execution::{MemoryLease, MemoryRelease},
    incident::*,
    lifecycle::*,
};
use sugarsubstitute_desktop::report::{NativeReportView, ReportSurfaceOptions};
struct Release(Arc<AtomicU64>);
impl MemoryRelease for Release {
    fn release(&self, bytes: u64) {
        self.0.fetch_add(bytes, Ordering::SeqCst);
    }
}
fn lease(bytes: u64) -> (MemoryLease, Arc<AtomicU64>) {
    let released = Arc::new(AtomicU64::new(0));
    (MemoryLease::new(bytes, Box::new(Release(released.clone()))), released)
}
fn binding() -> Result<ReportBinding, Box<dyn Error>> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    Ok(ReportBinding::new(
        IncidentKey::new(IncidentSubject::Supervisor(owner)),
        IncidentRevision::new(2)?,
        ReportSessionId::new(1)?,
    ))
}
fn view() -> Result<NativeReportView, Box<dyn Error>> {
    let (lease, _) = lease(512);
    Ok(NativeReportView::from_claimed(
        binding()?,
        "报告\n".into(),
        lease.claim(512)?,
        ReportRestartAvailability::Available,
    )?)
}
#[test]
fn inert_view_transfer_keeps_exact_bytes_binding_and_noncloneable_credit()
-> Result<(), Box<dyn Error>> {
    let (lease, released) = lease(512);
    let view = NativeReportView::from_claimed(
        binding()?,
        "报告\n".into(),
        lease.claim(512)?,
        ReportRestartAvailability::Available,
    )?;
    assert_eq!(view.binding(), binding()?);
    assert_eq!(view.text(), "报告\n");
    drop(lease);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    drop(view);
    assert_eq!(released.load(Ordering::SeqCst), 512);
    Ok(())
}
#[test]
fn malformed_native_text_rejects_and_releases_its_owned_credit() -> Result<(), Box<dyn Error>> {
    for text in ["", "missing final line", "embedded\0data\n", "wrong\r\n"] {
        let (lease, released) = lease(512);
        assert!(
            NativeReportView::from_claimed(
                binding()?,
                text.into(),
                lease.claim(512)?,
                ReportRestartAvailability::Available
            )
            .is_err()
        );
        drop(lease);
        assert_eq!(released.load(Ordering::SeqCst), 512);
    }
    Ok(())
}
#[test]
fn allocation_capacity_and_header_credit_are_checked_separately_from_length()
-> Result<(), Box<dyn Error>> {
    let (lease, _) = lease(1_000_000);
    let mut large = String::with_capacity(NativeReportView::MAX_BYTES + 1);
    large.push_str("small\n");
    assert!(
        NativeReportView::from_claimed(
            binding()?,
            large,
            lease.claim(1_000_000)?,
            ReportRestartAvailability::Available
        )
        .is_err()
    );
    assert!(
        NativeReportView::from_claimed(
            binding()?,
            "small\n".into(),
            lease.claim(6)?,
            ReportRestartAvailability::Available
        )
        .is_err()
    );
    Ok(())
}
#[test]
fn native_options_reject_insufficient_admission() -> Result<(), Box<dyn Error>> {
    let (short, _) = lease(ReportSurfaceOptions::KNOWN_STORAGE_BYTES - 1);
    assert!(ReportSurfaceOptions::new(view()?, "en", short).is_err());
    let (lease, _) = lease(ReportSurfaceOptions::KNOWN_STORAGE_BYTES);
    ReportSurfaceOptions::new(view()?, "unknown", lease)?;
    Ok(())
}
