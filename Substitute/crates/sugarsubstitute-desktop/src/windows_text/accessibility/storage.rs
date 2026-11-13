// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Charge the complete bounded authored UIA inventory before allocating or exporting interfaces.
use super::*;
use sugarsubstitute_contracts::presentation::NATIVE_CONSOLE_ACCESSIBILITY_BYTES;

pub(super) const MAX_LEASES: usize = 4096;
const RC_HEADER: usize = 2 * size_of::<usize>();
// windows-core 0.62.2 WeakRefCount's optional COM weak-reference tearoff has three pointers and
// two AtomicI32 counters. This conservatively includes one while its authored wrapper lives;
// caller-retained framework weak references may outlive every authored wrapper and its claim.
const WEAK_TEAROFF: usize = 3 * size_of::<usize>() + 2 * size_of::<std::sync::atomic::AtomicI32>();
const fn larger(left: usize, right: usize) -> usize {
    if left > right { left } else { right }
}
const WRAPPER: usize = larger(
    size_of::<text::Text_Impl>(),
    larger(size_of::<text::Value_Impl>(), size_of::<range::Range_Impl>()),
);
/// Native Msftedit providers and caller-owned BSTR/SAFEARRAY marshaling remain framework allocations.
pub(super) const AUTHORED_BYTES: usize = size_of::<Accessibility>()
    + RC_HEADER
    + size_of::<Shared>()
    + size_of::<provider::Container_Impl>()
    + size_of::<provider::TextElement_Impl>()
    + size_of::<provider::WindowlessSite_Impl>()
    + 3 * WEAK_TEAROFF
    + MAX_LEASES
        * (size_of::<Weak<Lease>>() + RC_HEADER + size_of::<Lease>() + WRAPPER + WEAK_TEAROFF);
const _: () = assert!(AUTHORED_BYTES as u64 <= NATIVE_CONSOLE_ACCESSIBILITY_BYTES);

/// Reserve one exact inventory so per-request admission never grows a Vec implicitly.
pub(super) fn inventory() -> windows::core::Result<Vec<Weak<Lease>>> {
    let mut leases = Vec::new();
    leases.try_reserve_exact(MAX_LEASES).map_err(|_| windows::core::Error::from(E_OUTOFMEMORY))?;
    if leases.capacity() != MAX_LEASES {
        return Err(E_OUTOFMEMORY.into());
    }
    Ok(leases)
}
