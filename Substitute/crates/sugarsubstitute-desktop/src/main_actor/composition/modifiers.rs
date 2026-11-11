// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Sample the keyboard modifiers Windows associates with the input message being processed.
//!
//! `GetKeyState` reports the key state synchronized with the calling thread's message queue, so
//! a read inside the window procedure observes the modifiers held when the current mouse or pen
//! message was generated rather than the live keyboard. Win32 mouse and pointer key-state words
//! carry Shift and Control but never Alt, so all three come from the same source.
//!
//! Between messages, such as while a held gesture waits without motion, the keyboard's live
//! state is read instead: keyboard messages go to the browser's focus window, so this thread's
//! queue state may not follow them.

use sugarsubstitute_canvas::ToolModifiers;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyState, VIRTUAL_KEY, VK_CONTROL, VK_MENU, VK_SHIFT,
};

/// Modifiers held for the message this UI thread is currently processing.
pub(super) fn current() -> ToolModifiers {
    from_key_states(key_state(VK_SHIFT), key_state(VK_CONTROL), key_state(VK_MENU))
}

/// Modifiers the keyboard holds now, independent of any message.
pub(super) fn live() -> ToolModifiers {
    from_key_states(live_state(VK_SHIFT), live_state(VK_CONTROL), live_state(VK_MENU))
}

fn key_state(key: VIRTUAL_KEY) -> i16 {
    // SAFETY: Copies this UI thread's queue-synchronized key state; nothing is retained.
    unsafe { GetKeyState(i32::from(key.0)) }
}

fn live_state(key: VIRTUAL_KEY) -> i16 {
    // SAFETY: Copies the keyboard's current key state; nothing is retained.
    unsafe { GetAsyncKeyState(i32::from(key.0)) }
}

/// Map `GetKeyState` or `GetAsyncKeyState` words to modifiers: a key is held when the word's
/// high bit is set.
/// The low toggle bit (for example after a Shift press) never counts as held.
pub(super) const fn from_key_states(shift: i16, ctrl: i16, alt: i16) -> ToolModifiers {
    ToolModifiers { shift: shift < 0, ctrl: ctrl < 0, alt: alt < 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_held_bit_marks_a_modifier_and_toggle_state_is_ignored() {
        let held = i16::MIN;
        assert_eq!(from_key_states(0, 0, 0), ToolModifiers::NONE);
        assert_eq!(from_key_states(1, 1, 1), ToolModifiers::NONE, "toggled but released");
        assert_eq!(
            from_key_states(held, 0, held | 1),
            ToolModifiers { shift: true, ctrl: false, alt: true }
        );
        assert_eq!(
            from_key_states(0, held, 0),
            ToolModifiers { shift: false, ctrl: true, alt: false }
        );
    }
}
