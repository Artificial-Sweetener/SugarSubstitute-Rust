// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exact borrowed v2 ABI storage; construction belongs to the actual execution owner.

/// Original cleanup counter observation from a synchronous borrowed callback.
#[repr(C)]
#[derive(Default)]
pub(crate) struct CleanupBudget {
    pub(super) version: u32,
    pub(super) size: u32,
    pub(super) state: u32,
    pub(super) reserved: u32,
    pub(super) started: i64,
    pub(super) grace: i64,
    pub(super) total: i64,
    pub(super) frequency: i64,
}
/// Borrowed callback context exists only through one synchronous native call.
#[repr(C)]
pub(crate) struct CleanupSource {
    pub(super) context: usize,
    pub(super) request: extern "C" fn(usize, *mut CleanupBudget) -> bool,
}
/// One synchronous exact-stage claim or nonblocking I/O issue callback.
#[repr(C)]
pub(crate) struct IssueSource {
    pub(super) context: usize,
    pub(super) request: extern "C" fn(usize, usize, u32, u64) -> u32,
}
/// Private fixed776-byte attachment input borrowed by the trusted native driver.
/// Callbacks, process capabilities and signal remain owned by Execution.
#[repr(C)]
pub struct NativeCaptureInput {
    pub(super) version: u32,
    pub(super) size: u32,
    pub(super) subject: u32,
    pub(super) provenance: u32,
    pub(super) namespace: [u8; 32],
    pub(super) epoch: [u8; 16],
    pub(super) instance: u64,
    pub(super) generation: u64,
    pub(super) creator_pid: u32,
    pub(super) handler_pid: u32,
    pub(super) creator_birth: u64,
    pub(super) handler_birth: u64,
    pub(super) creator_process: usize,
    pub(super) handler_process: usize,
    pub(super) attach_counter: i64,
    pub(super) frequency: i64,
    pub(super) reserved_deadline0: i64,
    pub(super) reserved_deadline1: i64,
    pub(super) build_length: u32,
    pub(super) endpoint_length: u32,
    pub(super) build: [u8; 64],
    pub(super) endpoint: [u16; 256],
    pub(super) cleanup: CleanupSource,
    pub(super) issue: IssueSource,
    pub(super) completion_event: usize,
}
#[cfg(windows)]
impl NativeCaptureInput {
    /// Compare inert build bytes with the actual authenticated selected bundle.
    /// This grants no process, endpoint, or admission authority.
    pub fn matches_build(&self, selected: &[u8]) -> bool {
        let length = self.build_length as usize;
        !selected.is_empty()
            && length <= self.build.len()
            && self.build.get(..length).is_some_and(|actual| actual == selected)
    }
    /// Build only from the native owner's sealed actual child/process/peer binding.
    /// All callbacks/event are borrowed for this one synchronous outer native call.
    pub(crate) fn from_captured(
        binding: &crate::windows_lifecycle_pipe::captured::child::CapturedChildBinding,
        cleanup: CleanupSource,
        issue: IssueSource,
        completion_event: usize,
    ) -> Self {
        let (facts, run) = binding.facts();
        Self {
            version: 2,
            size: size_of::<Self>() as u32,
            subject: 1,
            provenance: 1,
            namespace: *run.owner.namespace.bytes(),
            epoch: *run.owner.epoch.bytes(),
            instance: run.instance.get(),
            generation: run.generation.get(),
            creator_pid: facts.creator_pid,
            handler_pid: facts.handler_pid,
            creator_birth: facts.creator_birth,
            handler_birth: facts.handler_birth,
            creator_process: facts.processes[0],
            handler_process: facts.processes[1],
            attach_counter: facts.source.ticks(),
            frequency: facts.source.frequency(),
            reserved_deadline0: 0,
            reserved_deadline1: 0,
            build_length: facts.build_length as u32,
            endpoint_length: facts.endpoint_length as u32,
            build: facts.build,
            endpoint: facts.endpoint,
            cleanup,
            issue,
            completion_event,
        }
    }
}
/// Private fixed48-byte progress input with per-call borrowed callback sources.
#[repr(C)]
pub struct NativeCaptureProgress {
    pub(super) version: u32,
    pub(super) size: u32,
    pub(super) attach: usize,
    pub(super) cleanup: CleanupSource,
    pub(super) issue: IssueSource,
}
/// Exact callback-only32-byte one-shot stage token; never a scheduler capability.
#[repr(C)]
pub struct NativeCaptureStep {
    pub(super) version: u32,
    pub(super) size: u32,
    pub(super) attach: usize,
    pub(super) stage: u32,
    pub(super) reserved: u32,
    pub(super) nonce: u64,
}
/// Initialized120-byte output with custody adopted before any decoder/status use.
/// External drivers can forward its borrow but cannot manufacture its private facts.
#[repr(C)]
#[derive(Default)]
pub struct NativeCaptureReply {
    pub(super) version: u32,
    pub(super) status: u32,
    pub(super) registered: u32,
    pub(super) pending_io: u32,
    pub(super) retired: u32,
    pub(super) primary_error: u32,
    pub(super) attach: usize,
    pub(super) cleanup_count: u32,
    pub(super) cleanup_codes: [u32; 8],
    pub(super) primary_domain: u32,
    pub(super) omitted_cleanup: u64,
    pub(super) cleanup_state: u32,
    pub(super) cleanup_domains: u32,
    pub(super) cleanup_started: i64,
    pub(super) cleanup_grace: i64,
    pub(super) cleanup_total: i64,
    pub(super) frequency: i64,
}
const _: () = assert!(size_of::<usize>() == 8);
const _: () = assert!(size_of::<CleanupBudget>() == 48);
const _: () = assert!(size_of::<CleanupSource>() == 16);
const _: () = assert!(size_of::<IssueSource>() == 16);
const _: () = assert!(size_of::<NativeCaptureInput>() == 776);
const _: () = assert!(std::mem::offset_of!(NativeCaptureInput, cleanup) == 736);
const _: () = assert!(std::mem::offset_of!(NativeCaptureInput, issue) == 752);
const _: () = assert!(std::mem::offset_of!(NativeCaptureInput, completion_event) == 768);
const _: () = assert!(size_of::<NativeCaptureProgress>() == 48);
const _: () = assert!(size_of::<NativeCaptureStep>() == 32);
const _: () = assert!(size_of::<NativeCaptureReply>() == 120);
const _: () = assert!(std::mem::offset_of!(NativeCaptureReply, attach) == 24);
const _: () = assert!(std::mem::offset_of!(NativeCaptureReply, primary_domain) == 68);
const _: () = assert!(std::mem::offset_of!(NativeCaptureReply, omitted_cleanup) == 72);
const _: () = assert!(std::mem::offset_of!(NativeCaptureReply, cleanup_domains) == 84);
