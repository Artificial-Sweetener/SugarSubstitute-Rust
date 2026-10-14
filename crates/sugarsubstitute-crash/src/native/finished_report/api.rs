// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Resolve fixed finished exports through the existing current authenticated PIN loader.
use super::abi;
use crate::CrashError;
use std::ffi::CStr;
impl abi::Api {
    /// The actual Capture loader supplies its same module and current retention checker.
    /// The resolved pointers stay inside its existing permanently pinned native capsule.
    pub(in crate::native) fn resolve(
        symbol: &mut impl FnMut(
            &'static CStr,
        ) -> Result<unsafe extern "system" fn() -> isize, CrashError>,
    ) -> Result<Self, CrashError> {
        let open = symbol(c"ss_finished_open_v1")?;
        let read = symbol(c"ss_finished_read_v1")?;
        let retire = symbol(c"ss_finished_retire_v1")?;
        let prepare = symbol(c"ss_finished_prepare_disposition_v1")?;
        let discard = symbol(c"ss_finished_discard_v1")?;
        let recover = symbol(c"ss_finished_recover_disposition_v1")?;
        // SAFETY: Same authenticated permanent PIN and fixed reviewed ABI03/04 names.
        // x64 C signatures bind each current-source borrow to the synchronous call.
        Ok(unsafe {
            Self {
                open: std::mem::transmute::<unsafe extern "system" fn() -> isize, abi::OpenFn>(
                    open,
                ),
                read: std::mem::transmute::<unsafe extern "system" fn() -> isize, abi::ReadFn>(
                    read,
                ),
                retire: std::mem::transmute::<unsafe extern "system" fn() -> isize, abi::RetireFn>(
                    retire,
                ),
                prepare: std::mem::transmute::<unsafe extern "system" fn() -> isize, abi::PrepareFn>(
                    prepare,
                ),
                discard: std::mem::transmute::<unsafe extern "system" fn() -> isize, abi::DiscardFn>(
                    discard,
                ),
                recover: std::mem::transmute::<unsafe extern "system" fn() -> isize, abi::RecoverFn>(
                    recover,
                ),
            }
        })
    }
}
