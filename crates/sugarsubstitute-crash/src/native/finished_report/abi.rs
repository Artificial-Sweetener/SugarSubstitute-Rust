// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Match the finished-report ABI03 and cause-origin amendment04 exactly.
use super::control::ControlSource;

#[repr(C)]
#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub(super) struct Subject {
    pub tag: u32,
    pub namespace: [u8; 32],
    pub epoch: [u8; 16],
    pub reserved: u32,
    pub instance: u64,
    pub generation: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(super) struct Deadline {
    pub counter: i64,
    pub frequency: i64,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct Facts {
    pub subject: Subject,
    pub uuid: [u8; 16],
    pub build_length: u32,
    pub build: [u8; 64],
    pub reserved: u32,
    pub source_bytes: u64,
    pub source_identity: [u8; 24],
    pub temporary_bytes: u64,
    pub entries: u32,
    pub complete: u32,
}
impl Facts {
    pub(super) const fn empty() -> Self {
        Self {
            subject: Subject {
                tag: 0,
                namespace: [0; 32],
                epoch: [0; 16],
                reserved: 0,
                instance: 0,
                generation: 0,
            },
            uuid: [0; 16],
            build_length: 0,
            build: [0; 64],
            reserved: 0,
            source_bytes: 0,
            source_identity: [0; 24],
            temporary_bytes: 0,
            entries: 0,
            complete: 0,
        }
    }
}
#[repr(C)]
pub(super) struct Output {
    pub version: u32,
    pub status: u32,
    pub primary_error: u32,
    pub cleanup_count: u32,
    pub cleanup_codes: [u32; 8],
    pub omitted_cleanup: u64,
    pub lease: usize,
    pub retired: u32,
    pub handles: u32,
    pub locks: u32,
    pub primary_origin: u32,
    pub facts_valid: u32,
    pub cleanup_origin_bits: u32,
    pub facts: Facts,
    pub read_bytes: u64,
    pub reclaimed_bytes: u64,
}
impl Output {
    pub(super) const fn empty() -> Self {
        Self {
            version: 0,
            status: 0,
            primary_error: 0,
            cleanup_count: 0,
            cleanup_codes: [0; 8],
            omitted_cleanup: 0,
            lease: 0,
            retired: 0,
            handles: 0,
            locks: 0,
            primary_origin: 0,
            facts_valid: 0,
            cleanup_origin_bits: 0,
            facts: Facts::empty(),
            read_bytes: 0,
            reclaimed_bytes: 0,
        }
    }
}
#[repr(C)]
pub(super) struct Open<'call> {
    pub version: u32,
    pub reserved: u32,
    pub database: usize,
    pub metrics: usize,
    pub database_identity: [u8; 24],
    pub metrics_identity: [u8; 24],
    pub subject: Subject,
    pub build_length: u32,
    pub build: [u8; 64],
    pub optional_uuid: [u8; 16],
    pub has_uuid: u32,
    pub original: Deadline,
    pub current: ControlSource<'call>,
}
#[repr(C)]
pub(super) struct Call<'call> {
    pub version: u32,
    pub phase: u32,
    pub lease: usize,
    pub original: Deadline,
    pub current: ControlSource<'call>,
}
#[repr(C)]
pub(super) struct Read<'call> {
    pub call: Call<'call>,
    pub offset: u64,
    pub output: *mut u8,
    pub capacity: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub(super) struct Operation {
    pub namespace: [u8; 32],
    pub epoch: [u8; 16],
    pub id: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(super) struct FileFacts {
    pub identity: [u8; 24],
    pub bytes: u64,
    pub sha256: [u8; 32],
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(super) struct StageFacts {
    pub role: u32,
    pub reserved: u32,
    pub file: FileFacts,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct Preparation {
    pub version: u32,
    pub reserved: u32,
    pub historical: Operation,
    pub subject: Subject,
    pub uuid: [u8; 16],
    pub build_length: u32,
    pub build: [u8; 64],
    pub reserved_build: u32,
    pub database_identity: [u8; 24],
    pub metrics_identity: [u8; 24],
    pub metadata: FileFacts,
    pub source: FileFacts,
    pub original_stage: StageFacts,
    pub candidate_stage: StageFacts,
}
impl Preparation {
    pub(super) fn empty() -> Self {
        Self {
            version: 0,
            reserved: 0,
            historical: Operation::default(),
            subject: Subject::default(),
            uuid: [0; 16],
            build_length: 0,
            build: [0; 64],
            reserved_build: 0,
            database_identity: [0; 24],
            metrics_identity: [0; 24],
            metadata: FileFacts::default(),
            source: FileFacts::default(),
            original_stage: StageFacts::default(),
            candidate_stage: StageFacts::default(),
        }
    }
}
#[repr(C)]
pub(super) struct Prepare<'call> {
    pub call: Call<'call>,
    pub operation: Operation,
}
#[repr(C)]
pub(super) struct PrepareOutput {
    pub state: Output,
    pub valid: u32,
    pub reserved: u32,
    pub preparation: Preparation,
}
#[repr(C)]
pub(super) struct Disposition {
    pub prepared: Preparation,
    pub disposition: u32,
    pub reason: u32,
    pub revision: u64,
    pub summary: FileFacts,
    pub has_payload: u32,
    pub reserved_payload: u32,
    pub payload: FileFacts,
    pub intent: FileFacts,
    pub plan_sha256: [u8; 32],
    pub current: Operation,
    pub authorization: u32,
    pub reserved_authorization: u32,
}
#[repr(C)]
pub(super) struct Discard<'call> {
    pub call: Call<'call>,
    pub permit: *const Disposition,
}
#[repr(C)]
pub(super) struct Recover<'call> {
    pub version: u32,
    pub reserved: u32,
    pub database: usize,
    pub metrics: usize,
    pub database_identity: [u8; 24],
    pub metrics_identity: [u8; 24],
    pub permit: *const Disposition,
    pub original: Deadline,
    pub current: ControlSource<'call>,
}
#[repr(C)]
pub(super) struct DispositionOutput {
    pub state: Output,
    pub source: u32,
    pub metadata: u32,
    pub stages_absent: u32,
    pub reserved: u32,
}
const _: () = {
    assert!(size_of::<Subject>() == 72);
    assert!(size_of::<Facts>() == 208);
    assert!(size_of::<Output>() == 312);
    assert!(std::mem::offset_of!(Output, primary_origin) == 76);
    assert!(std::mem::offset_of!(Output, cleanup_origin_bits) == 84);
    assert!(size_of::<Open<'_>>() == 264);
    assert!(size_of::<Call<'_>>() == 48);
    assert!(size_of::<Read<'_>>() == 72);
    assert!(size_of::<Operation>() == 56);
    assert!(size_of::<FileFacts>() == 64);
    assert!(size_of::<StageFacts>() == 72);
    assert!(size_of::<Preparation>() == 544);
    assert!(size_of::<Prepare<'_>>() == 104);
    assert!(size_of::<PrepareOutput>() == 864);
    assert!(size_of::<Disposition>() == 856);
    assert!(size_of::<Discard<'_>>() == 56);
    assert!(size_of::<Recover<'_>>() == 112);
    assert!(size_of::<DispositionOutput>() == 328);
    assert!(std::mem::offset_of!(Call<'_>, current) == 32);
    assert!(std::mem::offset_of!(Open<'_>, current) == 248);
};

pub(super) type OpenFn = for<'a> unsafe extern "C" fn(*const Open<'a>, *mut Output);
pub(super) type ReadFn = for<'a> unsafe extern "C" fn(*const Read<'a>, *mut Output);
pub(super) type RetireFn = for<'a> unsafe extern "C" fn(*const Call<'a>, *mut Output);
pub(super) type PrepareFn = for<'a> unsafe extern "C" fn(*const Prepare<'a>, *mut PrepareOutput);
pub(super) type DiscardFn =
    for<'a> unsafe extern "C" fn(*const Discard<'a>, *mut DispositionOutput);
pub(super) type RecoverFn =
    for<'a> unsafe extern "C" fn(*const Recover<'a>, *mut DispositionOutput);

/// These fixed function pointers remain backed by the genuine process-lifetime PIN owner.
#[derive(Clone, Copy)]
pub(in crate::native) struct Api {
    pub(super) open: OpenFn,
    pub(super) read: ReadFn,
    pub(super) retire: RetireFn,
    pub(super) prepare: PrepareFn,
    pub(super) discard: DiscardFn,
    pub(super) recover: RecoverFn,
}
