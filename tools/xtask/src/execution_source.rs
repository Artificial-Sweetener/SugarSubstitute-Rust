// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Discover scheduling capability imports and references without scanning prose or literals.

use std::collections::BTreeSet;

use crate::error::Result;

/// Identify scheduler packages by canonical Cargo identity, independent of dependency aliases.
pub(crate) fn restricted_crate(name: &str) -> bool {
    matches!(
        name,
        "tokio"
            | "tokio_util"
            | "rayon"
            | "rayon_core"
            | "async_std"
            | "smol"
            | "async_executor"
            | "async_global_executor"
            | "async_io"
            | "blocking"
            | "threadpool"
            | "scheduled_thread_pool"
            | "futures"
            | "futures_executor"
            | "futures_util"
            | "futures_task"
            | "crossbeam"
            | "crossbeam_utils"
            | "crossbeam_channel"
            | "async_channel"
            | "flume"
            | "pollster"
    )
}

/// Reject raw thread/queue ownership and native scheduling entry points outside the adapter.
pub(crate) fn restricted_path(path: &[String]) -> bool {
    let Some(first) = path.first().map(String::as_str) else { return false };
    if restricted_crate(first) {
        return true;
    }
    let parts: Vec<_> = path.iter().map(String::as_str).collect();
    matches!(parts.as_slice(), ["std" | "core", "*"])
        || matches!(parts.as_slice(), ["std", "sync", "*"])
        || parts.starts_with(&["std", "thread"])
        || parts.starts_with(&["std", "sync", "mpsc"])
        || parts.windows(2).any(|pair| pair == ["glib", "MainContext"])
        || path.last().is_some_and(|name| {
            matches!(
                name.as_str(),
                "CreateThread"
                    | "CreateRemoteThread"
                    | "CreateRemoteThreadEx"
                    | "QueueUserWorkItem"
                    | "CreateThreadpoolWork"
                    | "CreateThreadpoolTimer"
                    | "CreateThreadpoolWait"
                    | "CreateThreadpoolIo"
                    | "StartThreadpoolIo"
                    | "SetThreadpoolTimer"
                    | "SetThreadpoolWait"
                    | "SetThreadpoolWaitEx"
                    | "RegisterWaitForSingleObject"
                    | "ConnectNamedPipe"
                    | "CancelIoEx"
                    | "GetOverlappedResult"
                    | "GetOverlappedResultEx"
                    | "SubmitThreadpoolWork"
                    | "TrySubmitThreadpoolCallback"
                    | "pthread_create"
                    | "pthread_detach"
                    | "thrd_create"
                    | "_beginthread"
                    | "_beginthreadex"
                    | "dispatch_async"
                    | "dispatch_async_f"
                    | "spawn_blocking"
                    | "spawn_local"
                    | "spawn_future"
                    | "spawn_future_local"
                    | "spawn_scoped"
                    | "spawn_ok"
                    | "spawn_obj"
            )
        })
}

/// Return deterministic source findings; invalid Rust never receives a clean inspection result.
pub(crate) fn findings(source: &str) -> Result<Vec<String>> {
    filtered_findings(source, restricted_path, scheduling_fragments)
}

/// Reject original execution resource construction and producer ports outside their two owners.
pub(crate) fn resource_producer_findings(source: &str) -> Result<Vec<String>> {
    filtered_findings(source, resource_producer_path, resource_fragments)
}
/// Detect private qualification creator construction, including aliases and macro fragments.
pub(crate) fn qualification_producer_findings(source: &str) -> Result<Vec<String>> {
    filtered_findings(
        source,
        |path| {
            path.windows(2).any(|pair| {
                pair[0] == "AuthenticatedDesktopCreator" && pair[1] == "from_authenticated"
            })
        },
        |names| {
            names.contains("AuthenticatedDesktopCreator") && names.contains("from_authenticated")
        },
    )
}
/// Test authority stays inside its exact Core-owner regression fixture, including macro aliases.
pub(crate) fn qualification_fixture_findings(source: &str) -> Result<Vec<String>> {
    filtered_findings(
        source,
        |path| {
            path.last().is_some_and(|name| name == "borrow_preview_owner_fixture")
                || path.windows(2).any(|pair| {
                    pair[0] == "AuthenticatedDesktopCreator" && pair[1] == "owner_fixture"
                })
        },
        |names| {
            names.contains("borrow_preview_owner_fixture")
                || (names.contains("AuthenticatedDesktopCreator")
                    && names.contains("owner_fixture"))
        },
    )
}

/// Test-only Report request metadata uses the same alias/macro scanner and a separate exact owner rule.
pub(crate) fn report_copy_fixture_findings(source: &str) -> Result<Vec<String>> {
    filtered_findings(
        source,
        |path| {
            path.last().is_some_and(|name| name == "with_pending_report_copy")
                || path.windows(2).any(|pair| pair[0] == "report" && pair[1] == "copy_fixture")
        },
        |names| {
            names.contains("with_pending_report_copy")
                || (names.contains("report") && names.contains("copy_fixture"))
        },
    )
}

fn resource_producer_path(path: &[String]) -> bool {
    path.windows(2).any(|pair| {
        (pair[0] == "InheritedCaptureAttachInput"
            && matches!(pair[1].as_str(), "authenticated" | "simulated"))
            || (pair[0] == "NativeCaptureInput" && pair[1] == "from_captured")
    }) || path.last().is_some_and(|name| name == "NativeHandleRelease")
        || path.windows(2).any(|pair| {
            matches!(
                pair[0].as_str(),
                "JobContext"
                    | "AdmittedJobResources"
                    | "NativeHandleLease"
                    | "MemoryLeaseObservation"
                    | "CaptureBundleAdmission"
                    | "CaptureBundleOrigin"
                    | "CaptureBundleRetirement"
                    | "ReportExportAdmission"
                    | "ReportExportOrigin"
                    | "ReportExportRetirement"
                    | "ReportExportOutput"
                    | "ReportExportCleanupDeadline"
                    | "RegisteredStartupSource"
                    | "RegisteredRestartAdmission"
                    | "PreparedStartupSnapshot"
            ) && pair[1] == "new"
        })
}
fn scheduling_fragments(names: &BTreeSet<String>) -> bool {
    names.contains("std") && (names.contains("thread") || names.contains("mpsc"))
}
fn resource_fragments(names: &BTreeSet<String>) -> bool {
    (names.contains("InheritedCaptureAttachInput")
        && (names.contains("authenticated") || names.contains("simulated")))
        || (names.contains("NativeCaptureInput") && names.contains("from_captured"))
        || names.contains("NativeHandleRelease")
        || names.contains("new")
            && names.iter().any(|name| {
                matches!(
                    name.as_str(),
                    "JobContext"
                        | "AdmittedJobResources"
                        | "NativeHandleLease"
                        | "MemoryLeaseObservation"
                        | "CaptureBundleAdmission"
                        | "CaptureBundleOrigin"
                        | "CaptureBundleRetirement"
                        | "ReportExportAdmission"
                        | "ReportExportOrigin"
                        | "ReportExportRetirement"
                        | "ReportExportOutput"
                        | "ReportExportCleanupDeadline"
                        | "RegisteredStartupSource"
                        | "RegisteredRestartAdmission"
                        | "PreparedStartupSnapshot"
                )
            })
}
/// Apply execution-specific capability rules through the shared Rust scanner.
fn filtered_findings(
    source: &str,
    restricted: fn(&[String]) -> bool,
    fragments: fn(&BTreeSet<String>) -> bool,
) -> Result<Vec<String>> {
    crate::capability_source::findings(source, restricted, fragments, restricted_crate)
}
