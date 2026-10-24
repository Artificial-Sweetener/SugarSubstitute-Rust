// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Construct the original same-table pending native family record.
use super::*;
impl Record {
    pub(super) fn pending(
        ticket: LaunchTicket,
        shared: SharedOwner,
        native_scope: Scope,
        root_spec: (sugarsubstitute_contracts::execution::ScopeId, JobId),
    ) -> Self {
        Self {
            establishment_job: None,
            establishment_completion: None,
            ticket,
            shared,
            native_scope,
            rejected_launch: false,
            stopped_before_exit: false,
            creation: None,
            root_spec,
            root_watch: None,
            root_notice: false,
            recovery_watch_issued: false,
            root: None,
            root_exit: None,
            last_member_exit: None,
            forced: false,
            root_retirement: None,
            member_watch: None,
            member_spec: None,
            member_notice: false,
            watched: None,
            pending_exit: None,
            cleanup: None,
            stop: None,
            retained: 0,
            next: None,
            closed_native: false,
            started: false,
            settled: false,
            error: None,
            failure_transferred: false,
            unread: [false; 5],
        }
    }
}
