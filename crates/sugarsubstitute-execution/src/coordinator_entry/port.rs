// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Describe actual external I/O and credential checks without exposing native custody.
use super::types::EntryTransportError;
use crate::{
    native::{NativeObservationError, Signal},
    native_deadlines::Role,
};
use std::sync::Arc;
use sugarsubstitute_contracts::{
    coordinator_entry::EntryNativeIdentityFacts,
    execution::{JobContext, Moment},
};

#[derive(Clone, Copy)]
pub(crate) struct KernelPeer {
    pub(crate) facts: EntryNativeIdentityFacts,
    pub(crate) pid: u32,
    pub(crate) birth: u64,
}
pub(crate) use crate::native_pipe::Progress;
pub(crate) trait EntryLease: Send {
    fn sample_counter(&mut self) -> Result<(u64, u64), NativeObservationError>;
    fn prepare(
        &mut self,
        signals: &[[Arc<Signal>; 3]],
        context: &JobContext,
        original_preparation: Moment,
    ) -> Result<(), EntryTransportError>;
    fn connect(
        &mut self,
        index: usize,
        issue: super::NativeIssue<'_>,
    ) -> Result<super::IssueDispatch<Progress>, EntryTransportError>;
    fn read(
        &mut self,
        index: usize,
        bytes: &mut [u8],
        issue: super::NativeIssue<'_>,
    ) -> Result<super::IssueDispatch<Progress>, EntryTransportError>;
    fn write(
        &mut self,
        index: usize,
        bytes: &[u8],
        issue: super::NativeIssue<'_>,
    ) -> Result<super::IssueDispatch<Progress>, EntryTransportError>;
    fn poll(&mut self, index: usize, role: Role) -> Result<Progress, EntryTransportError>;
    fn quiesce(&mut self, index: usize, role: Role) -> Result<(), EntryTransportError>;
    fn verify(&mut self, index: usize, message: bool) -> Result<KernelPeer, EntryTransportError>;
    fn nonce(&mut self) -> Result<[u8; 32], EntryTransportError>;
    fn retire_connection(&mut self, index: usize, reuse: bool)
    -> Result<bool, EntryTransportError>;
    fn retire(&mut self) -> Result<bool, EntryTransportError>;
}
