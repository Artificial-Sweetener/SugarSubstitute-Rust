// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Platform-independent selection previews and committed application values.
/// Native selection observation, independent of application commands or display labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionEvent {
    /// The popup began with this exact selected index.
    Open(Option<usize>),
    /// Native selection moved, either inside a popup or directly on a closed control.
    Changed,
    /// The user explicitly accepted the popup selection.
    Accept,
    /// The popup closed without accepting its preview.
    Cancel,
    /// Authoritative application data is about to replace the options or selection.
    Replace,
}
/// Effect for the native adapter; application work is admitted only on Commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionEffect {
    /// Keep a local native preview without notifying application state.
    None,
    /// Notify application state of a committed native selection.
    Commit,
    /// Restore the exact original native selection before yielding the UI thread.
    Restore(Option<usize>),
}
/// Retain one bounded popup gesture while the original selector and list remain owned.
#[derive(Default)]
pub struct SelectionGesture {
    original: Option<Option<usize>>,
}
impl SelectionGesture {
    /// Borrow the committed selection underlying a live preview; None means no popup gesture.
    pub fn original(&self) -> Option<Option<usize>> {
        self.original
    }

    /// Advance the native gesture without running callbacks or crossing an OS boundary.
    pub fn observe(&mut self, event: SelectionEvent) -> SelectionEffect {
        match event {
            SelectionEvent::Open(selected) => {
                self.original = Some(selected);
                SelectionEffect::None
            }
            SelectionEvent::Changed if self.original.is_none() => SelectionEffect::Commit,
            SelectionEvent::Accept if self.original.take().is_some() => SelectionEffect::Commit,
            SelectionEvent::Cancel => {
                self.original.take().map_or(SelectionEffect::None, SelectionEffect::Restore)
            }
            SelectionEvent::Replace => {
                self.original = None;
                SelectionEffect::None
            }
            _ => SelectionEffect::None,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn popup_preview_cancel_and_programmatic_replacement_keep_selection_authority() {
        let mut gesture = SelectionGesture::default();
        assert_eq!(gesture.observe(SelectionEvent::Changed), SelectionEffect::Commit);
        assert_eq!(gesture.observe(SelectionEvent::Open(Some(2))), SelectionEffect::None);
        assert_eq!(gesture.observe(SelectionEvent::Changed), SelectionEffect::None);
        assert_eq!(gesture.observe(SelectionEvent::Cancel), SelectionEffect::Restore(Some(2)));
        assert_eq!(gesture.observe(SelectionEvent::Cancel), SelectionEffect::None);
        gesture.observe(SelectionEvent::Open(None));
        assert_eq!(gesture.observe(SelectionEvent::Cancel), SelectionEffect::Restore(None));
        gesture.observe(SelectionEvent::Open(Some(3)));
        assert_eq!(gesture.observe(SelectionEvent::Accept), SelectionEffect::Commit);
        assert_eq!(gesture.observe(SelectionEvent::Accept), SelectionEffect::None);
        gesture.observe(SelectionEvent::Open(Some(4)));
        gesture.observe(SelectionEvent::Replace);
        assert_eq!(gesture.observe(SelectionEvent::Cancel), SelectionEffect::None);
    }
}
