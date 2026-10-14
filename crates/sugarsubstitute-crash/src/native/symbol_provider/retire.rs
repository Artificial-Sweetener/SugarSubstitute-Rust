// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt successful native closes under same actual paired cleanup before releasing credit.
use super::*;
use sugarsubstitute_execution::SymbolProviderRetirement;
use windows::Win32::Foundation::{CloseHandle, FreeLibrary, HANDLE, HMODULE};
impl DbgHelpSymbolProvider {
    /// Checked-close module/session/process/DLL in dependency order under paired cleanup.
    /// # Errors
    /// Returns identical owner on any failure, preserving partial outputs, charge and fence.
    pub fn retire(mut self, proof: &SymbolProviderRetirement) -> Result<(), Self> {
        if self.retire_in_place(proof).is_err() {
            return Err(self);
        }
        Ok(())
    }
    /// Checked-close native effects while retaining original lookup/cleanup causes.
    /// The settled owner still owns the original result/error allocation lifetime.
    /// # Errors
    /// Returns an inert category; original failed capsule and source remain here.
    pub fn retire_in_place(
        &mut self,
        proof: &SymbolProviderRetirement,
    ) -> Result<(), SymbolProviderFailureCode> {
        if let Err(cause) = self.retire_inner(proof) {
            let code = cause.code();
            if let Some(f) = self.fields.as_mut()
                && f.retirement.is_none()
            {
                f.retirement = Some(cause);
            }
            return Err(code);
        }
        Ok(())
    }
    fn retire_inner(&mut self, proof: &SymbolProviderRetirement) -> Result<(), ProviderCause> {
        let origin = self.original.as_ref().ok_or(ProviderCause::Identity)?;
        if !proof.matches_origin(origin) {
            return Err(ProviderCause::Identity);
        }
        proof.check().map_err(ProviderCause::Control)?;
        let Some(f) = self.fields.as_mut() else {
            return Ok(());
        };
        let process = f.process.map(|p| HANDLE(p as *mut core::ffi::c_void));
        if let Some(module) = f.module {
            proof.check().map_err(ProviderCause::Control)?;
            // Safety: actual initialized session, exact loaded module, retained API DLL.
            let result = unsafe {
                (f.api.as_ref().ok_or(ProviderCause::Identity)?.unload)(
                    process.ok_or(ProviderCause::Identity)?,
                    module,
                )
            }
            .ok()
            .map_err(ProviderCause::Native);
            if result.is_ok() {
                f.module = None;
                f.outputs[3] = None;
            }
            super::control::observed(result, proof.check(), &mut f.secondary)?;
        }
        if f.session {
            proof.check().map_err(ProviderCause::Control)?;
            // Safety: same actual process/session, after owned module unload, DLL still retained.
            let result = unsafe {
                (f.api.as_ref().ok_or(ProviderCause::Identity)?.cleanup)(
                    process.ok_or(ProviderCause::Identity)?,
                )
            }
            .ok()
            .map_err(ProviderCause::Native);
            if result.is_ok() {
                f.session = false;
                f.outputs[2] = None;
                f.outputs[3] = None;
            }
            super::control::observed(result, proof.check(), &mut f.secondary)?;
        }
        if let Some(process) = process {
            proof.check().map_err(ProviderCause::Control)?;
            // Safety: exact owned OpenProcess output, never pseudo/caller/attached process ownership.
            let result = unsafe { CloseHandle(process) }.map_err(ProviderCause::Native);
            if result.is_ok() {
                f.process = None;
                f.outputs[1] = None;
            }
            super::control::observed(result, proof.check(), &mut f.secondary)?;
        }
        if let Some(dll) = f.dll {
            proof.check().map_err(ProviderCause::Control)?;
            // Safety: all exported calls finished; exact owned System32 LoadLibrary output.
            let result = unsafe { FreeLibrary(HMODULE(dll as *mut core::ffi::c_void)) }
                .map_err(ProviderCause::Native);
            if result.is_ok() {
                f.dll = None;
                f.api = None;
                f.outputs[0] = None;
            }
            super::control::observed(result, proof.check(), &mut f.secondary)?;
        }
        f.outputs = std::array::from_fn(|_| None);
        if f.global {
            ACTIVE.store(false, Ordering::Release);
            f.global = false;
        }
        Ok(())
    }
}
