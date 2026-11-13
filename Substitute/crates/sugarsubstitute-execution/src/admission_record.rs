// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Construct the authoritative passive admitted record for every central job.
use super::*;
impl Record {
    pub(super) fn admitted(
        context: &JobContext,
        job: Arc<dyn ErasedJob>,
        kind: &RequestKind,
        work: Option<&crate::task::Work>,
    ) -> Self {
        // Only Windows native work seeds custody and deadline slots.
        #[cfg(not(any(windows, test)))]
        let _ = work;
        let spec = context.spec();
        let deadline = context.deadline();
        let memory =
            sugarsubstitute_contracts::execution::MemoryLeaseObservation::new(context.memory());
        Record {
            #[cfg(any(windows, test))]
            process_creation: matches!(&kind, RequestKind::ProcessCreation(_)),
            #[cfg(any(windows, test))]
            custody_deadlines: if matches!(&kind, RequestKind::BackendCustody { .. }) {
                Some(Box::new(crate::backend_custody::deadlines::Deadlines::new()))
            } else {
                None
            },
            memory: Some(memory),
            #[cfg(any(windows, test))]
            entry_setup: match &kind {
                RequestKind::EntryPreparation(setup) => Some(*setup),
                _ => None,
            },
            #[cfg(any(windows, test))]
            preparation_issued: false,
            preparation: None,
            #[cfg(any(windows, test))]
            registered_startup_issued: false,
            capture_bundle_issued: false,
            #[cfg(any(windows, test))]
            process_capture: None,
            #[cfg(any(windows, test))]
            capture_event_issued: false,
            #[cfg(any(windows, test))]
            capture_event_identity: None,
            #[cfg(any(windows, test))]
            desktop_capture_custody: match work {
                Some(crate::task::Work::Native(work)) => work.initial_desktop_custody(),
                _ => None,
            },
            #[cfg(any(windows, test))]
            symbol_original: match &kind {
                #[cfg(windows)]
                RequestKind::SymbolNative { original, .. } => Some(original.job_id()),
                _ => None,
            },
            report_export: matches!(&kind, RequestKind::ReportExportMain { .. }),
            report_export_issued: false,
            report_cleanup: match &kind {
                RequestKind::ReportExportCleanup { main_scope, main_spec } => {
                    Some(crate::report_export::cleanup::Binding {
                        main_scope: *main_scope,
                        main_spec: *main_spec,
                        paired: false,
                        original: None,
                    })
                }
                _ => None,
            },
            symbol_cleanup: match &kind {
                RequestKind::SymbolCleanup { main_scope, main_spec } => {
                    Some(crate::symbol_cleanup::Binding {
                        report_export_partner: None,
                        main_scope: *main_scope,
                        main_spec: *main_spec,
                        paired: false,
                        original: None,
                    })
                }
                _ => None,
            },
            #[cfg(any(windows, test))]
            native_custody: match work {
                Some(crate::task::Work::Native(work)) => work.initial_custody(),
                _ => None,
            },
            #[cfg(any(windows, test))]
            native_deadlines: match work {
                Some(crate::task::Work::Native(work)) => work.initial_deadlines(),
                _ => None,
            },
            deadline,
            scope_id: context.scope_id(),
            spec,
            phase: JobPhase::Queued,
            job,
            report_pair: None,
            retiring: false,
            acknowledged: false,
            terminal_published: false,
        }
    }
}
