// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind only fixed DbgHelp exports from the actual System32-loaded module.
use super::cause::ProviderCause;
use windows::{
    Win32::{
        Foundation::{HANDLE, HMODULE},
        System::{
            Diagnostics::Debug::{IMAGEHLP_MODULEW64, MODLOAD_DATA, SYMBOL_INFOW},
            LibraryLoader::GetProcAddress,
        },
    },
    core::{BOOL, PCSTR, PCWSTR},
};
type Initialize = unsafe extern "system" fn(HANDLE, PCWSTR, BOOL) -> BOOL;
type SetOptions = unsafe extern "system" fn(u32) -> u32;
type Load = unsafe extern "system" fn(
    HANDLE,
    HANDLE,
    PCWSTR,
    PCWSTR,
    u64,
    u32,
    *const MODLOAD_DATA,
    u32,
) -> u64;
type Info = unsafe extern "system" fn(HANDLE, u64, *mut IMAGEHLP_MODULEW64) -> BOOL;
type FromAddr = unsafe extern "system" fn(HANDLE, u64, *mut u64, *mut SYMBOL_INFOW) -> BOOL;
type Unload = unsafe extern "system" fn(HANDLE, u64) -> BOOL;
type Cleanup = unsafe extern "system" fn(HANDLE) -> BOOL;
pub(super) struct Api {
    pub(super) initialize: Initialize,
    pub(super) options: SetOptions,
    pub(super) load: Load,
    pub(super) info: Info,
    pub(super) from_addr: FromAddr,
    pub(super) unload: Unload,
    pub(super) cleanup: Cleanup,
}
impl Api {
    pub(super) fn bind(
        module: HMODULE,
        origin: &sugarsubstitute_execution::SymbolProviderOrigin,
        secondary: &mut Option<ProviderCause>,
    ) -> Result<Self, ProviderCause> {
        macro_rules! bind {($name:literal,$target:ty)=>{{
            origin.check_effect().map_err(ProviderCause::Control)?;
            // Safety: fixed ASCII export from actual retained System32 DbgHelp;
            // exact pinned Windows metadata signature, module lives through every call.
            let result=unsafe {GetProcAddress(module,PCSTR(concat!($name,"\0").as_ptr()))}.ok_or_else(||ProviderCause::Native(windows::core::Error::from_thread()));
            let pointer=super::control::observed(result,origin.check_effect(),secondary)?;
            // Safety: each named export uses the exact ABI above, no pointer escapes this owner.
            unsafe {std::mem::transmute::<unsafe extern "system" fn()->isize,$target>(pointer)}
        }}}
        Ok(Self {
            initialize: bind!("SymInitializeW", Initialize),
            options: bind!("SymSetOptions", SetOptions),
            load: bind!("SymLoadModuleExW", Load),
            info: bind!("SymGetModuleInfoW64", Info),
            from_addr: bind!("SymFromAddrW", FromAddr),
            unload: bind!("SymUnloadModule64", Unload),
            cleanup: bind!("SymCleanup", Cleanup),
        })
    }
}
