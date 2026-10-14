// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Synchronously borrow the same protected dump inode into the statically linked Crashpad parser.
//!
//! The sole native parser retains no callbacks or memory. All callback input/output
//! borrows live for this call; panics are contained before crossing C ABI. No handles,
//! paths, scheduler controls or native authority are serialized in these matching facts.
use crate::incident::committed_dump::{IncidentDumpReader, cause::DumpReadCause};
use std::{
    ffi::c_void,
    panic::{AssertUnwindSafe, catch_unwind},
};
use sugarsubstitute_contracts::{
    execution::{AdmissionError, PanicCause},
    incident::{DumpId, IncidentKey, IncidentSubject},
};
#[repr(C)]
struct Expected {
    version: u32,
    role: u32,
    build_length: u32,
    reserved: u32,
    instance: u64,
    generation: u64,
    file_size: u64,
    namespace: [u8; 32],
    epoch: [u8; 16],
    build: [u8; 64],
    report_uuid: [u8; 16],
}
#[repr(C)]
#[derive(Default)]
struct ResultFacts {
    version: u32,
    status: u32,
    cause: u32,
    reserved: u32,
    offset: u64,
    report_uuid: [u8; 16],
    module_index: u32,
    dictionaries: u32,
    entries: u32,
    reserved_end: u32,
}
const _: () = assert!(size_of::<Expected>() == 168 && size_of::<ResultFacts>() == 56);
#[link(name = "committed_dump_parser", kind = "static", modifiers = "-bundle")]
unsafe extern "C" {
    fn SugarParseCommittedDump(
        expected: *const Expected,
        context: *mut c_void,
        read: extern "C" fn(*mut c_void, u64, *mut u8, usize) -> u32,
        check: extern "C" fn(*mut c_void) -> u32,
        result: *mut ResultFacts,
    );
}
/// Actual categorical result from the same statically linked parser, with source offset.
/// These observations grant no native authority and contain no record bytes or paths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommittedParserError {
    status: u32,
    cause: u32,
    offset: u64,
}
impl CommittedParserError {
    /// Return the actual schema1 parser status; zero denotes validated success.
    pub const fn status(self) -> u32 {
        self.status
    }
    /// Return the actual bounded parser cause category.
    pub const fn cause(self) -> u32 {
        self.cause
    }
    /// Return the actual source offset in bytes.
    pub const fn offset(self) -> u64 {
        self.offset
    }
}
impl std::fmt::Display for CommittedParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Committed parser status {} at offset {}", self.status, self.offset)
    }
}
impl std::error::Error for CommittedParserError {}
/// Exact matching observations projected only after the actual native parser succeeds.
/// No file access, release trust or privilege is inferred from these copied values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommittedDumpIdentity {
    report_uuid: [u8; 16],
    module_index: u32,
    dictionaries: u32,
    entries: u32,
}
impl CommittedDumpIdentity {
    /// Return actual copied-report UUID bytes.
    pub const fn report_uuid(self) -> [u8; 16] {
        self.report_uuid
    }
    /// Return the actual module index containing the complete Crashpad identity dictionary.
    pub const fn module_index(self) -> u32 {
        self.module_index
    }
    /// Return the number of actual module annotation dictionaries inspected.
    pub const fn dictionaries(self) -> u32 {
        self.dictionaries
    }
    /// Return the number of actual annotation entries inspected.
    pub const fn entries(self) -> u32 {
        self.entries
    }
}
struct Borrow<'a> {
    reader: &'a mut IncidentDumpReader,
    failed: bool,
}
fn panic_cause(payload: &(dyn std::any::Any + Send)) -> PanicCause {
    let text = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or("Native parser callback panicked");
    let mut end = text.len().min(1024);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    PanicCause::new(text[..end].to_owned())
}
extern "C" fn read(context: *mut c_void, offset: u64, output: *mut u8, count: usize) -> u32 {
    // Safety: the linked synchronous parser passes the live Borrow, and requests
    // a writable stack buffer for exactly count bytes. No pointer survives return.
    let state = unsafe { &mut *context.cast::<Borrow<'_>>() };
    if output.is_null() || count > 65_536 {
        state.reader.record(DumpReadCause::Capacity);
        state.failed = true;
        return 1;
    }
    // Safety: the checked nonnull parser buffer is writable for exactly count bytes.
    let bytes = unsafe { std::slice::from_raw_parts_mut(output, count) };
    match catch_unwind(AssertUnwindSafe(|| state.reader.read_dump(offset, bytes))) {
        Ok(Ok(actual)) if actual == count => 0,
        Ok(Ok(_)) => {
            state.reader.record(DumpReadCause::Identity);
            state.failed = true;
            1
        }
        Ok(Err(cause)) => {
            state.reader.record(cause);
            state.failed = true;
            1
        }
        Err(payload) => {
            state.reader.record(DumpReadCause::Panicked(panic_cause(payload.as_ref())));
            state.failed = true;
            1
        }
    }
}
extern "C" fn check(context: *mut c_void) -> u32 {
    // Safety: same synchronous Borrow contract as read. Control is observed even
    // after a failed source read; the original first cause remains independently owned.
    let state = unsafe { &mut *context.cast::<Borrow<'_>>() };
    match catch_unwind(AssertUnwindSafe(|| state.reader.control())) {
        Ok(Ok(())) => 0,
        Ok(Err(cause)) => {
            let status = match &cause {
                DumpReadCause::Control(AdmissionError::DeadlineExpired { .. }) => 1,
                _ => 3,
            };
            state.reader.record(cause);
            state.failed = true;
            status
        }
        Err(payload) => {
            state.reader.record(DumpReadCause::Panicked(panic_cause(payload.as_ref())));
            state.failed = true;
            3
        }
    }
}
pub(crate) fn validate(
    reader: &mut IncidentDumpReader,
    key: IncidentKey,
    dump: DumpId,
    file_size: u64,
    build: [u8; 64],
    build_length: u32,
) -> Result<CommittedDumpIdentity, DumpReadCause> {
    let coordinator = key.subject().coordinator();
    let (role, instance, generation) = match key.subject() {
        IncidentSubject::Supervisor(_) => (0, 0, 0),
        IncidentSubject::Desktop(run) => (1, run.instance.get(), run.generation.get()),
    };
    let expected = Expected {
        version: 1,
        role,
        build_length,
        reserved: 0,
        instance,
        generation,
        file_size,
        namespace: *coordinator.namespace.bytes(),
        epoch: *coordinator.epoch.bytes(),
        build,
        report_uuid: *dump.bytes(),
    };
    let mut state = Borrow { reader, failed: false };
    let mut actual = ResultFacts::default();
    // Safety: statically linked authoritative parser, exact reprC layouts, synchronous
    // callback borrows and output lifetime. Both callbacks contain every Rust unwind.
    unsafe {
        SugarParseCommittedDump(
            &expected,
            (&mut state as *mut Borrow<'_>).cast(),
            read,
            check,
            &mut actual,
        );
    }
    if state.failed
        || actual.version != 1
        || actual.status != 0
        || actual.reserved != 0
        || actual.reserved_end != 0
        || actual.report_uuid != *dump.bytes()
        || actual.dictionaries == 0
        || actual.entries == 0
    {
        return Err(DumpReadCause::Parser(CommittedParserError {
            status: actual.status,
            cause: actual.cause,
            offset: actual.offset,
        }));
    }
    Ok(CommittedDumpIdentity {
        report_uuid: actual.report_uuid,
        module_index: actual.module_index,
        dictionaries: actual.dictionaries,
        entries: actual.entries,
    })
}

#[cfg(test)]
mod tests;
