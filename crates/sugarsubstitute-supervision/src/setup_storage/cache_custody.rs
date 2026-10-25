// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Match cache consumers to private named containment jobs without acquiring stop authority.
//! Job creation, admission and shutdown remain with the native process adapter. An existing job,
//! even with empty membership, is conservatively live. Only native object absence proves retirement.
use crate::native_claims::files::{Budget, Handle};
use std::io;
use std::sync::Arc;
use sugarsubstitute_contracts::execution::JobContext;
use windows::{
    Win32::System::{
        Com::CoCreateGuid, JobObjects::OpenJobObjectW, SystemServices::JOB_OBJECT_QUERY,
    },
    core::HSTRING,
};

/// Matching identity for one immutable native family; it grants no process or stop authority.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct CacheFamilyId(u128);
impl CacheFamilyId {
    /// Allocate before recording consumer pins and before any native process effect.
    pub(crate) fn new(context: &JobContext) -> io::Result<Self> {
        crate::setup_storage::check(context)?;
        // SAFETY: synchronous OS UUID generation needs no COM apartment or retained pointers.
        let identity = unsafe { CoCreateGuid() }.map_err(io::Error::other)?.to_u128();
        if identity == 0 {
            return Err(io::Error::other("Native cache family identity is empty"));
        }
        Ok(Self(identity))
    }
    /// Decode canonical bounded matching facts from a verified cache-control envelope.
    pub(crate) fn parse(value: &str) -> io::Result<Self> {
        if value.len() != 32
            || !value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Native cache family identity is malformed",
            ));
        }
        let value = u128::from_str_radix(value, 16).map_err(io::Error::other)?;
        if value == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Native cache family identity is empty",
            ));
        }
        Ok(Self(value))
    }
    /// Canonical stable descriptor spelling; never persist Debug formatting.
    pub(crate) fn encoded(self) -> String {
        format!("{:032x}", self.0)
    }
    /// Private cross-session job namespace used by creation and read-only lifetime observation.
    pub(crate) fn native_name(self) -> HSTRING {
        HSTRING::from(format!(r"Global\SugarSubstitute.Cache.Worker.{:032x}", self.0))
    }
    /// Observe only object absence. A zero membership count does not prove final native release.
    /// Call on an admitted storage worker; no retries or work are scheduled here.
    pub(crate) fn retired(self, context: &JobContext) -> io::Result<bool> {
        crate::setup_storage::check(context)?;
        let budget =
            Budget::Protected(Arc::new(context.memory().claim(4096).map_err(io::Error::other)?));
        // SAFETY: the stable UTF-16 name remains live through this synchronous call. QUERY grants
        // no termination or process-creation rights, and the returned handle is immediately owned.
        let opened = unsafe { OpenJobObjectW(JOB_OBJECT_QUERY, false, &self.native_name()) };
        match opened {
            Ok(handle) => {
                let mut probe = Handle::new(handle, budget);
                probe.close().map_err(io::Error::other)?;
                crate::setup_storage::check(context)?;
                Ok(false)
            }
            Err(cause) if cause.code().0 as u32 == 0x8007_0002 => Ok(true),
            Err(cause) => Err(io::Error::other(cause)),
        }
    }
}
