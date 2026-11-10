// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe the real failure capsules and original execution charges without native lookup effects.
use super::*;
use crate::incident::{
    DumpReadCode, SymbolFrameError,
    committed_dump::{cause::DumpReadCause, diagnostic_fixture},
    symbol_report::{cause::SymbolReportCause as Cause, source::DataOwner},
};
#[derive(Clone, Copy)]
enum Case {
    Frame,
    Provider,
    Read,
}
/// Keep the genuine provider/decoder owners while injecting only external failure observations.
fn run(case: Case) -> TestResult {
    let rig = Rig::new()?;
    let mut service = rig.reserved()?;
    let source = service.source.clone();
    let client = rig.client.clone();
    let held = service.held.as_ref().ok_or("held cleanup absent")?;
    let job = rig
        .client
        .submit_symbol_helper(&rig.main, service.main_spec, held, move |context| {
            let admission = client.symbol_provider_admission(&context).map_err(|_| ())?;
            let mut provider =
                crate::incident::DbgHelpSymbolProvider::new(admission).map_err(|_| ())?;
            let original = provider.original_origin().cloned().ok_or(())?;
            if matches!(case, Case::Provider) {
                provider.retain_native_fixture_failure(windows::core::Error::from_hresult(
                    windows::Win32::Foundation::ERROR_ACCESS_DENIED.to_hresult(),
                ));
            }
            let decode = context.memory().claim(786_432).map_err(|_| ())?;
            let (first, secondary, retirement) = if matches!(case, Case::Read) {
                (
                    Some(DumpReadCause::Identity),
                    Some(DumpReadCause::Control(AdmissionError::InvalidSymbolAdmission)),
                    Some(DumpReadCause::Capacity),
                )
            } else {
                (None, None, None)
            };
            let dump = diagnostic_fixture::owner(decode, first, secondary, retirement);
            if !matches!(case, Case::Read) {
                assert!(std::error::Error::source(&dump).is_none());
            }
            let mut data = DataOwner::new(provider, original);
            data.dump = Some(dump);
            data.first = Some(match case {
                Case::Frame => Cause::Frame(SymbolFrameError::CodeView),
                Case::Provider => {
                    Cause::Provider(crate::incident::SymbolProviderFailureCode::Native)
                }
                Case::Read => Cause::Dump(DumpReadCode::Identity),
            });
            data.retirement = Some(Cause::State);
            source.update(|state| {
                state.started = true;
                state.before_effects = false;
                state.data = Some(data);
            });
            Ok::<_, ()>(())
        })
        .map_err(|failure| failure.error)?;
    service.main = Some(job);
    service.submitted = true;
    loop {
        let count = rig.wake.count.lock().map_err(|_| "Wake poisoned")?;
        let before = *count;
        let batch = rig.client.drain_completions(NonZeroUsize::new(32).ok_or("zero drain")?)?;
        service.capture(&batch);
        if service.main_terminal {
            drop(count);
            break;
        }
        drop(
            rig.wake
                .changed
                .wait_while(count, |count| *count == before)
                .map_err(|_| "Wake poisoned")?,
        );
    }
    assert!(service.advance_cleanup()?);
    rig.settle(&mut service)?;
    let result = service.take_result().ok_or("missing failure outcome")?;
    let SymbolReportResult::Failed(failure) = &result else {
        return Err("failure accepted".into());
    };
    assert!(failure.settled());
    let first = failure.source().ok_or("primary source missing")?;
    match case {
        Case::Frame => {
            assert!(matches!(
                first.downcast_ref::<Cause>(),
                Some(Cause::Frame(SymbolFrameError::CodeView))
            ));
            assert_eq!(first.to_string(), "Captured frame rejected: CodeView");
            assert!(failure.dump_failures().iter().all(Option::is_none));
        }
        Case::Provider => {
            assert!(first.downcast_ref::<crate::incident::DbgHelpSymbolProvider>().is_some());
            let actual = first
                .source()
                .and_then(|cause| cause.source())
                .and_then(|cause| cause.downcast_ref::<windows::core::Error>())
                .ok_or("original provider native cause hidden")?;
            assert_eq!(actual.code(), windows::Win32::Foundation::ERROR_ACCESS_DENIED.to_hresult());
            assert!(failure.dump_failures().iter().all(Option::is_none));
            assert_eq!(
                failure.provider_failures()[0].as_ref().map(|cause| cause.code()),
                Some(crate::incident::SymbolProviderFailureCode::Native)
            );
        }
        Case::Read => {
            assert!(first.downcast_ref::<crate::incident::IncidentDumpReadLease>().is_some());
            assert_eq!(
                first.source().ok_or("actual read cause hidden")?.to_string(),
                "Committed dump Identity"
            );
            assert_eq!(
                failure.dump_failures().map(|cause| cause.map(|cause| cause.code())),
                [
                    Some(DumpReadCode::Identity),
                    Some(DumpReadCode::Control),
                    Some(DumpReadCode::Capacity)
                ]
            );
        }
    }
    assert!(matches!(failure.causes().last(), Some(Cause::State)));
    drop(service);
    drop(rig.owner);
    assert_eq!(rig.main.snapshot().reserved_bytes, 8_159_232);
    assert_eq!(rig.cleanup.snapshot().reserved_bytes, 32_768);
    drop(result);
    assert_eq!(rig.main.snapshot().reserved_bytes, 0);
    assert_eq!(rig.cleanup.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn healthy_retired_dump_custody_does_not_mask_actual_later_frame_category() -> TestResult {
    run(Case::Frame)
}
#[test]
fn healthy_retired_dump_custody_does_not_mask_retained_provider_native_cause() -> TestResult {
    run(Case::Provider)
}
#[test]
fn primary_read_post_read_and_retirement_causes_remain_independently_observable() -> TestResult {
    run(Case::Read)
}
#[test]
fn finite_symbol_categories_format_without_arbitrary_native_payloads() {
    for (cause, expected) in [
        (Cause::Frame(SymbolFrameError::Corrupt), "Captured frame rejected: Corrupt"),
        (
            Cause::Provider(crate::incident::SymbolProviderFailureCode::Identity),
            "Symbol provider rejected: Identity",
        ),
        (Cause::Dump(DumpReadCode::Capacity), "Committed dump rejected: Capacity"),
        (Cause::State, "Symbol report required owner unavailable"),
    ] {
        assert_eq!(cause.to_string(), expected);
    }
}
