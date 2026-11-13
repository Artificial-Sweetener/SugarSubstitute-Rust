// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Confine raw environment allocation/read/Unicode Free to one exclusive Windows owner.

mod native;
pub(crate) use native::WindowsEnvironment;

#[cfg(test)]
mod native_abi_tests;
