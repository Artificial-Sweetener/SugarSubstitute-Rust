// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Map canvas interaction intent to shared cursor resources supplied by Windows.
use super::*;
use ::windows::Win32::UI::WindowsAndMessaging::{HCURSOR, IDC_ARROW, IDC_HAND, LoadCursorW};
use sugarsubstitute_canvas::CursorIntent;

/// Shared system cursors remain owned by Windows for the process lifetime.
pub(super) struct Cursors {
    arrow: HCURSOR,
    hand: HCURSOR,
}
impl Cursors {
    /// Load the current Windows cursor-scheme resources without creating private artwork.
    pub(super) fn new() -> Result<Self, Box<dyn Error>> {
        // SAFETY: Both identifiers request shared system resources that callers must not destroy.
        let arrow = unsafe { LoadCursorW(None, IDC_ARROW)? };
        // Windows supplies no stock open-grab/closed-grabbing pair. Both panning states use its
        // documented link-selection hand without embedding or manufacturing private cursor pixels.
        // SAFETY: IDC_HAND requests a shared Windows cursor; its handle remains system-owned.
        let hand = unsafe { LoadCursorW(None, IDC_HAND)? };
        Ok(Self { arrow, hand })
    }
    /// Resolve both panning states to the shared Windows hand resource.
    pub(super) const fn get(&self, intent: CursorIntent) -> HCURSOR {
        match intent {
            CursorIntent::Arrow => self.arrow,
            CursorIntent::OpenHand | CursorIntent::ClosedHand => self.hand,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn panning_intents_use_the_shared_windows_hand_handle() -> Result<(), Box<dyn Error>> {
        let cursors = Cursors::new()?;
        // SAFETY: Compare against the same shared system resources; no returned handle is owned.
        let arrow = unsafe { LoadCursorW(None, IDC_ARROW)? };
        // SAFETY: The same documented shared cursor remains owned by Windows.
        let hand = unsafe { LoadCursorW(None, IDC_HAND)? };
        assert_eq!(cursors.get(CursorIntent::Arrow), arrow);
        assert_eq!(cursors.get(CursorIntent::OpenHand), hand);
        assert_eq!(cursors.get(CursorIntent::ClosedHand), hand);
        Ok(())
    }
}
