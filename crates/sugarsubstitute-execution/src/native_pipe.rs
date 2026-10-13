// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Share bounded native pipe storage, callback retirement and token inspection.
#[cfg(windows)]
pub(crate) mod accounting;
#[cfg(windows)]
mod errors;
#[cfg(windows)]
pub(crate) mod event;
#[cfg(windows)]
pub(crate) mod handles;
#[cfg(windows)]
pub(crate) mod overlapped;
#[cfg(windows)]
pub(crate) mod security;
#[cfg(windows)]
pub(crate) use errors::NativePipeError;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Progress {
    Pending,
    Complete(u32),
}
