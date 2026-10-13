// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Consume the sole physical read core without file lookup or caller context renewal.
use super::image::ReadAt;
use crate::{ClaimError, protected_storage::native_file::NativeReadCore};

impl ReadAt for NativeReadCore<'_, '_, '_> {
    type Error = ClaimError;
    fn length(&self) -> u64 {
        NativeReadCore::length(self)
    }
    fn read_at(&mut self, offset: u64, output: &mut [u8]) -> Result<(), Self::Error> {
        self.read_exact(offset, output)
    }
}
