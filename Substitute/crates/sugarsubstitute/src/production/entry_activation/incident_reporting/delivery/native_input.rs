// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fund the native launch slot only inside the actual original512KiB preparation worker.
use super::*;

// Cover the allocated slot and its transient construction value before allocation.
// The enclosing charged draft already accounts this owner and its claim fields.
pub(in crate::production::entry_activation::incident_reporting) const NATIVE_INPUT_PEAK_BYTES: u64 =
    (2 * size_of::<Option<WindowsLaunchInput>>() + 2 * size_of::<usize>()) as u64;

/// The native slot deallocates before its original worker charge returns reusable credit.
/// This grants no native effects, family ownership, authentication or retirement.
pub(in crate::production::entry_activation::incident_reporting) struct NativeLaunchInput {
    value: Box<Option<WindowsLaunchInput>>,
    _storage: MemoryClaim,
}
impl NativeLaunchInput {
    /// Claim the complete slot construction peak before allocation or any caller transfer.
    /// Rejection leaves the existing original slot unchanged; repeated calls retain it.
    pub(in crate::production::entry_activation::incident_reporting) fn reserve(
        context: &JobContext,
        slot: &mut Option<Self>,
    ) -> std::result::Result<(), MemoryClaimError> {
        if slot.is_none() {
            let storage = context.memory().claim(NATIVE_INPUT_PEAK_BYTES)?;
            *slot = Some(Self { value: Box::new(None), _storage: storage });
        }
        Ok(())
    }
    /// Borrow the complete original inert native-input owner without separating its claim.
    pub(in crate::production::entry_activation::incident_reporting) fn input(
        &self,
    ) -> &Option<WindowsLaunchInput> {
        &self.value
    }
    /// Lend the original Option to the actual Parent's checked transfer boundary.
    /// A rejected Parent transfer leaves its command, environment and pins in this slot.
    pub(in crate::production::entry_activation::incident_reporting) fn input_mut(
        &mut self,
    ) -> &mut Option<WindowsLaunchInput> {
        &mut self.value
    }
}
