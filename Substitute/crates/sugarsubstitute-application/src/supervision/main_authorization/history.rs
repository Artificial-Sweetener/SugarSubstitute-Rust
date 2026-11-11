// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain exact request fingerprints with a nonwrapping lifetime high-water mark.
use super::MAIN_AUTHORIZATION_HISTORY;
use std::num::NonZeroU32;
use sugarsubstitute_contracts::lifecycle::{LifecycleFailureCode, LifecycleRejection, RequestId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Fingerprint {
    Folder(u32),
    Environment,
    Controller(NonZeroU32),
    Retired,
    Revealed,
    Failure(LifecycleFailureCode),
}
#[derive(Clone, Copy)]
struct Record {
    request: RequestId,
    fingerprint: Fingerprint,
    progress: Progress,
}
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Progress {
    Accepted,
    Verified,
    Issued,
}
pub(super) struct RequestHistory {
    records: [Option<Record>; MAIN_AUTHORIZATION_HISTORY],
    next: usize,
    highwater: u64,
}
impl Default for RequestHistory {
    fn default() -> Self {
        Self { records: [None; MAIN_AUTHORIZATION_HISTORY], next: 0, highwater: 0 }
    }
}
impl RequestHistory {
    pub(super) fn check(
        &self,
        request: RequestId,
        fingerprint: Fingerprint,
    ) -> Result<bool, LifecycleRejection> {
        if let Some(record) = self.records.iter().flatten().find(|r| r.request == request) {
            if record.fingerprint != fingerprint {
                return Err(LifecycleRejection::ChangedDuplicate);
            }
            return Ok(true);
        }
        if request.get() <= self.highwater {
            return Err(LifecycleRejection::StaleRequest);
        }
        Ok(false)
    }
    pub(super) fn record(&mut self, request: RequestId, fingerprint: Fingerprint) {
        self.records[self.next] =
            Some(Record { request, fingerprint, progress: Progress::Accepted });
        self.next = (self.next + 1) % MAIN_AUTHORIZATION_HISTORY;
        self.highwater = request.get();
    }
    pub(super) fn progress(
        &self,
        request: RequestId,
        fingerprint: Fingerprint,
    ) -> Option<Progress> {
        self.records
            .iter()
            .flatten()
            .find(|r| r.request == request && r.fingerprint == fingerprint)
            .map(|r| r.progress)
    }
    pub(super) fn advance(&mut self, request: RequestId, progress: Progress) {
        if let Some(record) = self.records.iter_mut().flatten().find(|r| r.request == request) {
            record.progress = progress;
        }
    }
    pub(super) fn contains(&self, request: RequestId) -> bool {
        self.records.iter().flatten().any(|r| r.request == request)
    }
    pub(super) fn clear_records(&mut self) {
        self.records.fill(None);
        self.next = 0;
    }
    pub(super) const fn highwater(&self) -> u64 {
        self.highwater
    }
}
