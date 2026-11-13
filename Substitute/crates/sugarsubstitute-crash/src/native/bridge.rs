// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exact v1 ABI; a process-lifetime pin precedes every possible callback installation.
use super::module_pin::{LegacyLoan, SLOT};
use crate::CrashError;
use sugarsubstitute_contracts::{crash::CaptureOperation, execution::MemoryClaim};
use windows::{
    Win32::{Foundation::HMODULE, System::LibraryLoader::*},
    core::{PCSTR, PCWSTR},
};

#[repr(C)]
pub(super) struct Input {
    pub(super) handler: *const u16,
    pub(super) handler_length: usize,
    pub(super) database: *const u16,
    pub(super) database_length: usize,
    pub(super) metrics: *const u16,
    pub(super) metrics_length: usize,
    pub(super) build: *const u8,
    pub(super) build_length: usize,
    pub(super) coordinator: *const u8,
    pub(super) coordinator_length: usize,
    pub(super) deadline_counter: i64,
    pub(super) counter_frequency: i64,
}
#[repr(C)]
#[derive(Default)]
pub(super) struct ResultValue {
    pub(super) version: u32,
    pub(super) registered: u32,
    pub(super) error: u32,
    pub(super) cleanup_error: u32,
    pub(super) pending_io: u32,
    pub(super) retired: u32,
    pub(super) process: usize,
    pub(super) thread: usize,
    pub(super) cleanup_count: u32,
    pub(super) cleanup_codes: [u32; 8],
    pub(super) omitted_cleanup: u64,
}
type Start = unsafe extern "C" fn(*const Input, *mut ResultValue);
type Retire = unsafe extern "C" fn(*mut ResultValue);
#[repr(C)]
struct EndpointExpectation {
    version: u32,
    creator_pid: u32,
    creator_birth: u64,
    handler_pid: u32,
    reserved: u32,
    handler_birth: u64,
}
#[repr(C)]
struct EndpointValue {
    version: u32,
    status: u32,
    provenance: u32,
    length: u32,
    creator_pid: u32,
    handler_pid: u32,
    creator_birth: u64,
    handler_birth: u64,
    error: u32,
    reserved: u32,
    endpoint: [u16; 256],
}
type Endpoint = unsafe extern "C" fn(*const EndpointExpectation, *mut EndpointValue) -> bool;
const _: () = assert!(size_of::<EndpointExpectation>() == 32);
const _: () = assert!(size_of::<EndpointValue>() == 560);
const _: () = assert!(std::mem::offset_of!(EndpointValue, endpoint) == 48);
#[derive(Clone, Copy)]
pub(super) struct Api {
    start: Start,
    retire: Retire,
    endpoint: Option<Endpoint>,
    pub(super) finished: super::finished_report::Api,
}
/// Preserve the genuine loader cause and any outputs not installed in the PIN slot.
/// No new rejection allocation or replayable trust authority is created.
type LoadFailure = (
    CrashError,
    Option<MemoryClaim>,
    Option<Box<sugarsubstitute_supervision::installed_release::CaptureBundleParts>>,
);
pub(super) fn load(
    parts: Box<sugarsubstitute_supervision::installed_release::CaptureBundleParts>,
    memory: MemoryClaim,
) -> Result<Api, LoadFailure> {
    let path = match parts
        .retention
        .client_path()
        .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))
        .and_then(super::pins::wide)
    {
        Ok(path) => path,
        Err(cause) => return Err((cause, Some(memory), Some(parts))),
    };
    let mut loan = match LegacyLoan::reserve(&SLOT, memory) {
        Ok(loan) => loan,
        Err(memory) => {
            return Err((
                CrashError::invalid(CaptureOperation::Runtime),
                Some(memory),
                Some(parts),
            ));
        }
    };
    if let Err(parts) = loan.install_bundle(parts) {
        let callback = loan.return_unused().map(|value| value.0);
        return Err((CrashError::invalid(CaptureOperation::Runtime), callback, Some(parts)));
    }
    let current = loan
        .bundle()
        .ok_or_else(|| CrashError::invalid(CaptureOperation::Runtime))
        .and_then(|parts| {
            parts
                .loaded
                .check_effect()
                .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))
        });
    if let Err(cause) = current {
        let returned = loan.return_unused();
        let (callback, parts) =
            returned.map_or((None, None), |(callback, parts)| (Some(callback), parts));
        return Err((cause, callback, parts));
    }
    // SAFETY: the same canonical no-write/no-delete file remains retained in the installed capsule.
    // UTF16 storage and System32-only dependency lookup are bounded by the admitted native claim.
    let module = match unsafe {
        LoadLibraryExW(PCWSTR(path.as_ptr()), None, LOAD_LIBRARY_SEARCH_SYSTEM32)
    } {
        Ok(module) => module,
        Err(cause) => {
            let returned = loan.return_unused();
            let (callback, parts) =
                returned.map_or((None, None), |(callback, parts)| (Some(callback), parts));
            return Err((CrashError::new(CaptureOperation::Runtime, cause), callback, parts));
        }
    };
    loan.adopt_module(module.0 as usize);
    if let Err(cause) = loan
        .bundle()
        .ok_or_else(|| CrashError::invalid(CaptureOperation::Runtime))
        .and_then(|parts| {
            parts
                .pinned
                .check_effect()
                .map_err(|cause| CrashError::new(CaptureOperation::Runtime, cause))
        })
    {
        return Err((cause, None, None));
    }
    loan.pin_attempt();
    let mut pinned = HMODULE::default();
    // SAFETY: the exact loaded reference, canonical name and whole verified capsule remain held.
    let result = unsafe {
        GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_PIN, PCWSTR(path.as_ptr()), &mut pinned)
    };
    if let Err(cause) = result {
        return Err((CrashError::new(CaptureOperation::Runtime, cause), None, None));
    }
    if pinned.0.is_null() || pinned != module {
        return Err((CrashError::invalid(CaptureOperation::Runtime), None, None));
    }
    let mut symbol=|name:&'static std::ffi::CStr|->Result<unsafe extern "system" fn()->isize,CrashError> {
        loan.bundle().ok_or_else(||CrashError::invalid(CaptureOperation::Runtime))?.retention.check_effect().map_err(|cause|CrashError::new(CaptureOperation::Runtime,cause))?;
        // SAFETY: fixed reviewed names are resolved from the identical process-pinned DLL.
        unsafe {GetProcAddress(module,PCSTR(name.as_ptr().cast()))}.ok_or_else(||CrashError::new(CaptureOperation::Runtime,windows::core::Error::from_thread()))
    };
    let start = match symbol(c"ss_capture_start_v1") {
        Ok(value) => value,
        Err(cause) => return Err((cause, None, None)),
    };
    let retire = match symbol(c"ss_capture_retire_io_v1") {
        Ok(value) => value,
        Err(cause) => return Err((cause, None, None)),
    };
    let endpoint = match symbol(c"ss_capture_endpoint_v1") {
        Ok(value) => value,
        Err(cause) => return Err((cause, None, None)),
    };
    let finished = match super::finished_report::Api::resolve(&mut symbol) {
        Ok(value) => value,
        Err(cause) => return Err((cause, None, None)),
    };
    // SAFETY: the authenticated reviewed first-party module implements these exact repr(C) exports.
    Ok(unsafe {
        Api {
            finished,
            start: std::mem::transmute::<unsafe extern "system" fn() -> isize, Start>(start),
            retire: std::mem::transmute::<unsafe extern "system" fn() -> isize, Retire>(retire),
            endpoint: Some(std::mem::transmute::<unsafe extern "system" fn() -> isize, Endpoint>(
                endpoint,
            )),
        }
    })
}
impl Api {
    pub(super) fn endpoint(
        self,
        expected: sugarsubstitute_execution::windows_capture_attach::CaptureAttachExpectation,
    ) -> Result<
        sugarsubstitute_execution::windows_capture_attach::CaptureEndpointObservation,
        CrashError,
    > {
        use sugarsubstitute_execution::windows_capture_attach::{
            CaptureEndpointObservation, CaptureEndpointProvenance, CaptureProcessFacts,
        };
        let getter =
            self.endpoint.ok_or_else(|| CrashError::invalid(CaptureOperation::Observation))?;
        let input = EndpointExpectation {
            version: 1,
            creator_pid: expected.creator().process_id(),
            creator_birth: expected.creator().creation_time_100ns(),
            handler_pid: expected.handler().process_id(),
            reserved: 0,
            handler_birth: expected.handler().creation_time_100ns(),
        };
        let mut output = EndpointValue {
            version: 0,
            status: 0,
            provenance: 0,
            length: 0,
            creator_pid: 0,
            handler_pid: 0,
            creator_birth: 0,
            handler_birth: 0,
            error: 0,
            reserved: 0,
            endpoint: [0; 256],
        };
        // SAFETY: Process-pinned getter borrows exactly these initialized fixed
        // ABI buffers; it performs no callback, allocation, load or kernel query.
        let success = unsafe { getter(&input, &mut output) };
        if output.error != 0 {
            return Err(CrashError::new(
                CaptureOperation::Observation,
                std::io::Error::from_raw_os_error(output.error as i32),
            ));
        }
        if !success
            || output.version != 1
            || output.status != 1
            || output.provenance != 1
            || output.reserved != 0
            || output.length == 0
            || output.length > 256
            || output.endpoint[output.length.min(256) as usize..].iter().any(|unit| *unit != 0)
        {
            return Err(CrashError::invalid(CaptureOperation::Observation));
        }
        let creator = CaptureProcessFacts::new(output.creator_pid, output.creator_birth)
            .map_err(|cause| CrashError::new(CaptureOperation::Observation, cause))?;
        let handler = CaptureProcessFacts::new(output.handler_pid, output.handler_birth)
            .map_err(|cause| CrashError::new(CaptureOperation::Observation, cause))?;
        CaptureEndpointObservation::new(
            &expected,
            CaptureEndpointProvenance::CreatorInherited,
            creator,
            handler,
            &output.endpoint[..output.length as usize],
        )
        .map_err(|cause| CrashError::new(CaptureOperation::Observation, cause))
    }
    pub(super) fn start(self, input: &Input) -> ResultValue {
        let mut output = ResultValue::default();
        // SAFETY: Borrowed arrays remain valid for this finite synchronous call;
        // initialized output transfers all handles before its status is interpreted.
        unsafe { (self.start)(input, &mut output) };
        output
    }
    pub(super) fn retire(self) -> ResultValue {
        let mut output = ResultValue::default();
        // SAFETY: Module/context remain process-pinned; completion storage never moves.
        unsafe { (self.retire)(&mut output) };
        output
    }
}
pub(super) fn pinned_bytes() -> u64 {
    SLOT.legacy_bytes()
}
pub(super) fn pinned_reservation_bytes() -> u64 {
    SLOT.legacy_reservation()
}
