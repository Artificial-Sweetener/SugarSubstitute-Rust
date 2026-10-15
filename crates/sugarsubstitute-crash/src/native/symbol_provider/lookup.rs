// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve only the actual captured address against the pinned authenticated local PE/PDB.
use super::*;
#[path = "lookup/matching.rs"]
mod matching;
use crate::incident::CapturedExceptionFrame;
use std::{os::windows::ffi::OsStrExt, path::Path};
use sugarsubstitute_contracts::incident::{
    BuildIdentity, FileLabel, PdbIdentity, ReleaseSymbols, SymbolArchitecture, SymbolName,
};
use sugarsubstitute_supervision::installed_release::SymbolBundleLease;
use windows::{
    Win32::{
        Foundation::HANDLE,
        System::{
            Diagnostics::Debug::*,
            LibraryLoader::{LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW},
            Threading::{
                GetCurrentProcessId, OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
            },
        },
    },
    core::{BOOL, PCWSTR},
};
fn wide(path: &Path, output: &mut [u16]) -> Result<(), ProviderCause> {
    let mut count = 0;
    for word in path.as_os_str().encode_wide() {
        if word == 0 || count + 1 >= output.len() {
            return Err(ProviderCause::Identity);
        }
        output[count] = word;
        count += 1;
    }
    output[count] = 0;
    Ok(())
}
impl DbgHelpSymbolProvider {
    /// Resolve one validated exception using only actual authenticated selected files.
    /// Original provider/selection checks bracket each native operation; no symbol server,
    /// dump-supplied path, environment search or process invasion is enabled.
    /// # Errors
    /// Actual native/control causes remain with this owner and require checked cleanup.
    pub fn resolve(
        &mut self,
        selected: &SymbolBundleLease,
        frame: CapturedExceptionFrame,
        dump: sugarsubstitute_contracts::incident::DumpId,
    ) -> Result<SymbolFrameObservation, SymbolProviderFailureCode> {
        if self.construction.is_some() {
            return Err(SymbolProviderFailureCode::Control);
        }
        let result = self.resolve_inner(selected, frame, dump);
        result.map_err(|cause| self.record(cause))
    }
    fn resolve_inner(
        &mut self,
        selected: &SymbolBundleLease,
        frame: CapturedExceptionFrame,
        dump: sugarsubstitute_contracts::incident::DumpId,
    ) -> Result<SymbolFrameObservation, ProviderCause> {
        let origin = self.original.as_ref().ok_or(ProviderCause::Identity)?;
        origin.check_effect().map_err(ProviderCause::Control)?;
        selected.check_effect().map_err(ProviderCause::Control)?;
        let mine = origin.original_context().map_err(ProviderCause::Control)?;
        let other =
            selected.original_origin().original_context().map_err(ProviderCause::Control)?;
        if mine.owner_id() != other.owner_id()
            || mine.scope_id() != other.scope_id()
            || mine.job_id() != other.job_id()
        {
            return Err(ProviderCause::Identity);
        }
        let selected_facts = selected.registration_facts().map_err(ProviderCause::Selection)?;
        if selected_facts.pdb_guid() != *frame.pdb().guid()
            || selected_facts.pdb_age() != frame.pdb().age()
            || selected_facts.image_size() != frame.image_size()
        {
            return Err(ProviderCause::Identity);
        }
        let f = self.fields.as_mut().ok_or(ProviderCause::Identity)?;
        if f.first.is_some() || f.global {
            return Err(ProviderCause::Identity);
        }
        if ACTIVE.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_err() {
            return Err(ProviderCause::Busy);
        }
        f.global = true;
        f.outputs[0] = Some(origin.reserve_output().map_err(ProviderCause::Control)?);
        origin.check_effect().map_err(ProviderCause::Control)?;
        let dll_name = [100, 98, 103, 104, 101, 108, 112, 46, 100, 108, 108, 0];
        // Safety: fixed NUL-terminated system library name; search is restricted to System32.
        let result = unsafe {
            LoadLibraryExW(PCWSTR(dll_name.as_ptr()), None, LOAD_LIBRARY_SEARCH_SYSTEM32)
        }
        .map_err(ProviderCause::Native);
        if let Ok(dll) = &result {
            f.dll = Some(dll.0 as usize);
        }
        let dll = super::control::observed(result, origin.check_effect(), &mut f.secondary)?;
        f.api = Some(Api::bind(dll, origin, &mut f.secondary)?);
        f.outputs[1] = Some(origin.reserve_output().map_err(ProviderCause::Control)?);
        origin.check_effect().map_err(ProviderCause::Control)?;
        // Safety: current process only; this owned handle grants query/read, never termination.
        let result = unsafe {
            OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, GetCurrentProcessId())
        }
        .map_err(ProviderCause::Native);
        if let Ok(process) = &result {
            f.process = Some(process.0 as usize);
        }
        let process = super::control::observed(result, origin.check_effect(), &mut f.secondary)?;
        let parent = selected
            .pdb_path()
            .map_err(ProviderCause::Selection)?
            .parent()
            .ok_or(ProviderCause::Identity)?;
        wide(parent, &mut f.search)?;
        wide(selected.pdb_path().map_err(ProviderCause::Selection)?, &mut f.image)?;
        let api = f.api.as_ref().ok_or(ProviderCause::Identity)?;
        f.outputs[2] = Some(origin.reserve_output().map_err(ProviderCause::Control)?);
        origin.check_effect().map_err(ProviderCause::Control)?;
        selected.check_effect().map_err(ProviderCause::Control)?;
        // Safety: serialized process-wide DbgHelp options, only local exact-symbol lookup.
        unsafe {
            (api.options)(
                SYMOPT_EXACT_SYMBOLS
                    | SYMOPT_NO_PROMPTS
                    | SYMOPT_SECURE
                    | SYMOPT_UNDNAME
                    | SYMOPT_LOAD_LINES
                    | SYMOPT_IGNORE_NT_SYMPATH
                    | SYMOPT_NO_IMAGE_SEARCH
                    | SYMOPT_NO_UNQUALIFIED_LOADS
                    | SYMOPT_DISABLE_SYMSRV_AUTODETECT,
            )
        };
        origin.check_effect().map_err(ProviderCause::Control)?;
        selected.check_effect().map_err(ProviderCause::Control)?;
        // Latch possible session custody before the native call, including a failing partial return.
        f.session = true;
        // Safety: actual retained current-process handle and bounded selected-only search path.
        let result = unsafe { (api.initialize)(process, PCWSTR(f.search.as_ptr()), BOOL(0)) }
            .ok()
            .map_err(ProviderCause::Native);
        super::control::observed(
            result,
            origin.check_effect().and_then(|()| selected.check_effect()),
            &mut f.secondary,
        )?;
        f.outputs[3] = Some(origin.reserve_output().map_err(ProviderCause::Control)?);
        // Loading the exact pinned PDB directly avoids following the PE embedded build-time path.
        // Safety: authenticated PE/PDB pair, captured actual nonzero base/size, no callbacks.
        let base = unsafe {
            (api.load)(
                process,
                HANDLE::default(),
                PCWSTR(f.image.as_ptr()),
                PCWSTR::null(),
                frame.image_base(),
                frame.image_size(),
                std::ptr::null(),
                0,
            )
        };
        let result = if base == 0 {
            Err(ProviderCause::Native(windows::core::Error::from_thread()))
        } else {
            f.module = Some(base);
            Ok(base)
        };
        let base = super::control::observed(
            result,
            origin.check_effect().and_then(|()| selected.check_effect()),
            &mut f.secondary,
        )?;
        if base != frame.image_base() {
            return Err(ProviderCause::Identity);
        }
        let mut info = IMAGEHLP_MODULEW64 {
            SizeOfStruct: size_of::<IMAGEHLP_MODULEW64>() as u32,
            ..Default::default()
        };
        // Safety: initialized session and owned module; exact fixed metadata output layout.
        let result =
            unsafe { (api.info)(process, base, &mut info) }.ok().map_err(ProviderCause::Native);
        super::control::observed(
            result,
            origin.check_effect().and_then(|()| selected.check_effect()),
            &mut f.secondary,
        )?;
        let pdb_name = String::from_utf16(
            &info.LoadedPdbName[..info
                .LoadedPdbName
                .iter()
                .position(|v| *v == 0)
                .ok_or(ProviderCause::Identity)?],
        )
        .map_err(ProviderCause::Utf16)?;
        if !matching::direct_pdb(
            &info,
            frame.pdb(),
            frame.image_base(),
            frame.image_size(),
            selected.pdb_path().map_err(ProviderCause::Selection)?,
            &pdb_name,
        ) {
            return Err(ProviderCause::Identity);
        }
        let symbol = f.symbol.as_mut_ptr().cast::<SYMBOL_INFOW>();
        // Safety: 8-byte aligned buffer covers exact header plus512 UTF16 name words.
        unsafe {
            (*symbol).SizeOfStruct = size_of::<SYMBOL_INFOW>() as u32;
            (*symbol).MaxNameLen = 512;
        }
        let mut displacement = 0;
        origin.check_effect().map_err(ProviderCause::Control)?;
        selected.check_effect().map_err(ProviderCause::Control)?;
        // Safety: initialized local-only exact session, captured actual address and owned output buffer.
        let result =
            unsafe { (api.from_addr)(process, frame.address(), &mut displacement, symbol) }
                .ok()
                .map_err(ProviderCause::Native);
        super::control::observed(
            result,
            origin.check_effect().and_then(|()| selected.check_effect()),
            &mut f.secondary,
        )?;
        // Safety: successful API header and bounded fixed storage; length validated before slice creation.
        let length = unsafe { (*symbol).NameLen } as usize;
        if length == 0 || length > 512 {
            return Err(ProviderCause::Identity);
        }
        // Safety: name storage starts inside the same aligned buffer and covers validated length.
        let words = unsafe { std::slice::from_raw_parts((*symbol).Name.as_ptr(), length) };
        let name = String::from_utf16(words).map_err(ProviderCause::Utf16)?;
        let label = selected
            .image_path()
            .map_err(ProviderCause::Selection)?
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or(ProviderCause::Identity)?;
        let release = ReleaseSymbols::new(
            BuildIdentity::new(selected_facts.build().map_err(ProviderCause::Selection)?)
                .map_err(ProviderCause::Facts)?,
            FileLabel::new(label).map_err(ProviderCause::Facts)?,
            SymbolArchitecture::X86_64,
            PdbIdentity::new(selected_facts.pdb_guid(), selected_facts.pdb_age())
                .map_err(ProviderCause::Facts)?,
            selected_facts.image_sha256(),
            selected_facts.pdb_sha256(),
        );
        let frame = FailingFrame::new(
            dump,
            release,
            frame.address(),
            frame.address() - frame.image_base(),
            SymbolName::new(&name).map_err(ProviderCause::Facts)?,
            None,
        );
        Ok(SymbolFrameObservation { frame, _original: origin.clone() })
    }
}
