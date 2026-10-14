// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe a real predecessor mutation between pinned open and no-replace activation.
use super::*;
use crate::native::report_export::Destination;
use crate::report_export::ReportExportCause;
struct Probe {
    destination: Destination,
    failure: Option<ReportExportCause>,
}
/// The original namespace/summary remains a boundary fixture; only receiving effects qualify.
pub(in crate::report_export::operation::tests) fn predecessor_mutation(path: &Path) -> ResultT {
    use std::os::windows::ffi::OsStrExt;
    let current = summary()?;
    let binding = ReportBinding::new(
        current.summary().seed().key(),
        current.summary().revision(),
        ReportSessionId::new(1)?,
    );
    let bytes: Vec<u8> = path.as_os_str().encode_wide().flat_map(u16::to_le_bytes).collect();
    let intent = ReportExportIntent::new(
        binding,
        DiagnosticSelection {
            id: DiagnosticRequestId::new(1)?,
            selection: ExportSelection::TextOnly,
            overwrite: true,
            destination: &bytes,
        },
    )?;
    let clock = MonotonicClock::new();
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let main_scope =
        client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let cleanup_scope =
        client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let main_spec = spec(
        2,
        JobClass::InfrastructureOrdinary,
        131_072,
        Some(clock.now().checked_add(Duration::from_secs(30))?),
    )?;
    let shared: Arc<Mutex<Option<Probe>>> = Arc::new(Mutex::new(None));
    let cleanup_shared = shared.clone();
    let cleanup_client = client.clone();
    let permit = client
        .reserve_report_export_cleanup(
            &cleanup_scope,
            spec(1, JobClass::InfrastructureLifecycle, 32_768, None)?,
            &main_scope,
            main_spec,
            move |context| {
                let mut shared =
                    cleanup_shared.lock().map_err(|_| io::Error::other("probe poisoned"))?;
                if let Some(probe) = shared.as_mut() {
                    let proof = cleanup_client
                        .report_export_retirement(
                            probe.destination.origin().map_err(io::Error::other)?,
                            &context,
                        )
                        .map_err(io::Error::other)?;
                    probe.destination.retire(&proof).map_err(io::Error::other)?;
                }
                Ok::<_, io::Error>(true)
            },
        )
        .map_err(|failure| failure.error)?;
    let main_shared = shared.clone();
    let main_client = client.clone();
    let path = path.to_owned();
    let mut main = client
        .submit_report_export(&main_scope, main_spec, &permit, move |context| {
            let admission =
                main_client.report_export_admission(&context).map_err(io::Error::other)?;
            let mut shared = main_shared.lock().map_err(|_| io::Error::other("probe poisoned"))?;
            *shared = Some(Probe {
                destination: Destination::new(admission.into_origin()),
                failure: None,
            });
            let probe = shared.as_mut().ok_or_else(|| io::Error::other("probe absent"))?;
            probe.destination.open(&intent).map_err(io::Error::other)?;
            probe.destination.write(b"new selected bytes").map_err(io::Error::other)?;
            probe.destination.flush().map_err(io::Error::other)?;
            let mut permissions = std::fs::metadata(&path)?.permissions();
            permissions.set_readonly(true);
            std::fs::set_permissions(&path, permissions)?;
            probe.failure = probe.destination.activate().err();
            Ok::<_, io::Error>(matches!(
                probe.failure,
                Some(ReportExportCause::Rejected(C::Identity))
            ))
        })
        .map_err(|failure| failure.error)?;
    let main_completion = complete(&client, &wake, &mut main)?;
    {
        let shared = shared.lock().map_err(|_| "probe poisoned")?;
        if let Some(probe) = shared.as_ref() {
            let (_, stage, predecessor, activated) = probe.destination.custody();
            assert!(stage && predecessor && !activated);
        }
    }
    let deadline =
        client.freeze_report_export_cleanup_deadline(&permit, Duration::from_secs(15))?;
    let mut cleanup = permit.begin(deadline).map_err(|failure| failure.error)?;
    let cleanup_completion = complete(&client, &wake, &mut cleanup)?;
    assert!(matches!(cleanup_completion.outcome, JobOutcome::Success(true)));
    let probe = shared.lock().map_err(|_| "probe poisoned")?.take().ok_or("probe missing")?;
    assert!(probe.destination.closed());
    assert_eq!(probe.destination.custody().0, 0);
    match main_completion.outcome {
        JobOutcome::Success(true) => {}
        JobOutcome::Failure(cause) => return Err(Box::new(cause)),
        other => {
            return Err(format!("native predecessor mutation was not rejected: {other:?}").into());
        }
    }
    drop(probe);
    drop(main_completion.memory);
    drop(cleanup_completion.memory);
    assert_eq!(main_scope.snapshot().reserved_bytes, 0);
    assert_eq!(cleanup_scope.snapshot().reserved_bytes, 0);
    Ok(())
}
