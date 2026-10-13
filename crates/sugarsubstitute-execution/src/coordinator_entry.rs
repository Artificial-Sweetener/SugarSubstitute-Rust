// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own entry transport correlation, fixed storage and kernel-authenticated observations.
mod admission;
mod buffers;
mod control;
mod issue;
mod observation;
mod port;
mod prepared_cause;
pub(crate) mod prepared_failure;
mod protocol;
pub use prepared_cause::{EntryPreparedCause, EntryPreparedCauseKind};
pub use prepared_failure::EntryPreparedRetirement;
mod replay;
mod schedule;
mod startup_snapshot;
mod state;
mod types;
pub(crate) use issue::{IssueDispatch, NativeIssue};
#[cfg(test)]
mod tests;
pub(crate) mod worker;
use crate::{
    admission::Core,
    native::{NativeObservationError, Signal},
    scope::lock,
    task::Job,
};
pub(crate) use admission::{EntryWork, reserve_client, reserve_server};
pub use buffers::AuthenticatedEntryLaunch;
pub use control::{
    CoordinatorEntryClientSession, CoordinatorEntryControl, CoordinatorEntryServerSession,
};
pub use observation::EntryTransportObservation;
pub(crate) use port::{EntryLease, KernelPeer, Progress};
pub use startup_snapshot::{PreparedStartupSnapshot, StartupSnapshotRejection};
use state::{Command, Connection, Reply, RequestRoute, Shared, State};
use std::sync::{Arc, Mutex, Weak};
use sugarsubstitute_contracts::execution::{JobContext, Moment};
pub use types::*;
pub(crate) fn initial_deadlines(work: &EntryWork) -> crate::native_deadlines::Deadlines {
    crate::native_deadlines::Deadlines::handshake(work.shared.original_preparation)
}
