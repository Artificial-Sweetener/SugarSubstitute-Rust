// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode exact cause domains and inert facts after the retained owner adopts outputs.
use super::native_abi::NativeCaptureReply;
use std::{error::Error, fmt};

/// Preserve the native producer domain without interpreting errno/NTSTATUS as Win32.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CauseDomain {
    Win32,
    NtStatus,
    Errno,
}
/// Exact producer cause; formatting does not query or rewrite native thread error state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct NativeCause {
    pub(super) domain: CauseDomain,
    pub(super) code: u32,
}
impl fmt::Display for NativeCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Capture {:?} cause 0x{:08x}", self.domain, self.code)
    }
}
impl Error for NativeCause {}
/// Reject malformed ABI shape without releasing any original native custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReplyError {
    Version,
    Status,
    Boolean,
    CauseDomain,
    CleanupLedger,
    CleanupBudget,
    ContradictoryFacts,
}
impl fmt::Display for ReplyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Capture native reply rejected: {self:?}")
    }
}
impl Error for ReplyError {}
/// Bounded decoded observations; none grants application eligibility or closure.
pub(super) struct ReplyFacts {
    pub(super) status: u32,
    pub(super) registered: bool,
    pub(super) pending: bool,
    pub(super) retired: bool,
    pub(super) primary: Option<NativeCause>,
    pub(super) secondary: [Option<NativeCause>; 8],
    pub(super) omitted: u64,
    pub(super) cleanup: Option<CleanupCounters>,
}
/// Original latched cleanup counters, not a duration to restart at receipt.
#[derive(Clone, Copy)]
pub(super) struct CleanupCounters {
    pub(super) started: i64,
    pub(super) grace: i64,
    pub(super) total: i64,
    pub(super) frequency: i64,
}
/// Validate one exact domain/code pair without constructing a native Win32 error.
fn cause(code: u32, domain: u32) -> Result<Option<NativeCause>, ReplyError> {
    if code == 0 && domain == 0 {
        return Ok(None);
    }
    let domain = match domain {
        1 if code != 0 => CauseDomain::Win32,
        2 if (code as i32) < 0 => CauseDomain::NtStatus,
        3 if code > 0 && code <= i32::MAX as u32 => CauseDomain::Errno,
        _ => return Err(ReplyError::CauseDomain),
    };
    Ok(Some(NativeCause { domain, code }))
}
/// Validate only after raw nested/outer outputs are installed in the actual owner.
/// Malformed replies return a typed cause; no handle/token is closed or dereferenced.
pub(super) fn decode(reply: &NativeCaptureReply) -> Result<ReplyFacts, ReplyError> {
    if reply.version != 2 {
        return Err(ReplyError::Version);
    }
    if reply.status > 5 {
        return Err(ReplyError::Status);
    }
    if reply.registered > 1 || reply.pending_io > 1 || reply.retired > 1 {
        return Err(ReplyError::Boolean);
    }
    let registered = reply.registered == 1;
    let pending = reply.pending_io == 1;
    let retired = reply.retired == 1;
    if (retired && pending)
        || ((registered || pending || retired) && reply.attach == 0)
        || (reply.status == 0 && (reply.attach != 0 || registered || pending || retired))
        || (reply.status == 1 && (!registered || retired))
        || (reply.status == 4 && !retired)
        || (reply.status == 5 && (reply.attach == 0 || registered || retired))
    {
        return Err(ReplyError::ContradictoryFacts);
    }
    let primary = cause(reply.primary_error, reply.primary_domain)?;
    if reply.cleanup_count > 8 || reply.cleanup_domains >> (reply.cleanup_count * 2) != 0 {
        return Err(ReplyError::CleanupLedger);
    }
    let mut secondary = [None; 8];
    for (index, slot) in secondary.iter_mut().enumerate() {
        if index < reply.cleanup_count as usize {
            *slot = cause(reply.cleanup_codes[index], (reply.cleanup_domains >> (index * 2)) & 3)?;
            if slot.is_none() {
                return Err(ReplyError::CleanupLedger);
            }
        } else if reply.cleanup_codes[index] != 0 {
            return Err(ReplyError::CleanupLedger);
        }
    }
    if primary.is_none() && (reply.cleanup_count != 0 || reply.omitted_cleanup != 0) {
        return Err(ReplyError::CleanupLedger);
    }
    let cleanup = match reply.cleanup_state {
        0 if reply.cleanup_started == 0
            && reply.cleanup_grace == 0
            && reply.cleanup_total == 0
            && reply.frequency >= 0 =>
        {
            None
        }
        1 if reply.cleanup_started >= 0
            && reply.cleanup_grace > reply.cleanup_started
            && reply.cleanup_total > reply.cleanup_grace
            && reply.frequency > 0 =>
        {
            Some(CleanupCounters {
                started: reply.cleanup_started,
                grace: reply.cleanup_grace,
                total: reply.cleanup_total,
                frequency: reply.frequency,
            })
        }
        _ => return Err(ReplyError::CleanupBudget),
    };
    Ok(ReplyFacts {
        status: reply.status,
        registered,
        pending,
        retired,
        primary,
        secondary,
        omitted: reply.omitted_cleanup,
        cleanup,
    })
}

#[cfg(test)]
#[path = "native_reply_tests.rs"]
mod tests;
