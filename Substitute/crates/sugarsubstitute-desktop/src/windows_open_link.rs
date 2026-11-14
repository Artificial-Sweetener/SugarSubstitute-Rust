// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Open the page's links to the web in the user's default browser, never in a window of the
//! embedded browser.
//!
//! A link the page opens in a new window, such as a commit link in the release notes, reaches
//! the main browser's new-window request. Only plain `http` and `https` links with a host and
//! without embedded credentials are handed to the shell, so a link can never start a program
//! or reach a local file. Every request is cancelled inside the embedded browser.
use crate::DesktopError;
use windows::{
    Win32::UI::{
        Shell::{SEE_MASK_ASYNCOK, SEE_MASK_FLAG_NO_UI, SHELLEXECUTEINFOW, ShellExecuteExW},
        WindowsAndMessaging::SW_SHOWNORMAL,
    },
    core::{HSTRING, w},
};

/// The link the shell may open for a new-window request to `uri`, or `None` for any other
/// scheme, a link without a host and a link with credentials.
pub(crate) fn external_link(uri: &str) -> Option<url::Url> {
    let url = url::Url::parse(uri).ok()?;
    (matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some_and(|host| !host.is_empty())
        && url.username().is_empty()
        && url.password().is_none())
    .then_some(url)
}

/// What the embedded browser does with a new-window request from the page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NewWindow {
    /// Open no window inside the embedded browser.
    Cancel,
}

/// Answer one new-window request from the page: hand an allowed link to `open`, log a link
/// that is refused or that `open` could not open, and cancel the embedded window either way.
pub(crate) fn new_window(
    uri: &str,
    open: impl FnOnce(&url::Url) -> Result<(), DesktopError>,
) -> NewWindow {
    match external_link(uri) {
        Some(url) => {
            if let Err(cause) = open(&url) {
                tracing::warn!(operation = "open_link", cause = %cause,
                    "A link from the page could not be opened in the browser");
            }
        }
        None => tracing::warn!(
            operation = "open_link_refused",
            "A link from the page was refused: only http and https links open"
        ),
    }
    NewWindow::Cancel
}

/// Ask the shell to open `url` in the default browser. The shell returns without waiting for
/// the browser, so the calling turn stays short.
/// # Errors
/// Preserves the shell's failure.
pub(crate) fn open(url: &url::Url) -> Result<(), DesktopError> {
    let file = HSTRING::from(url.as_str());
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_ASYNCOK | SEE_MASK_FLAG_NO_UI,
        lpVerb: w!("open"),
        lpFile: windows::core::PCWSTR(file.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    // SAFETY: Every borrowed string lives across this synchronous call; the shell copies them.
    unsafe { ShellExecuteExW(&raw mut info) }.map_err(|cause| DesktopError::new("open_link", cause))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn only_plain_web_links_reach_the_shell() {
        for accepted in ["https://github.com/example/commit/abc1234", "http://example.test/notes"] {
            assert_eq!(external_link(accepted).as_ref().map(url::Url::as_str), Some(accepted));
        }
        for refused in [
            "file:///C:/Windows/System32/calc.exe",
            "ms-settings:privacy",
            "javascript:alert(1)",
            "about:blank",
            "data:text/html,hi",
            "https://user:secret@github.com/example",
            "https://user@github.com/example",
            "not a link",
        ] {
            assert!(external_link(refused).is_none(), "{refused}");
        }
    }

    #[test]
    fn every_new_window_is_cancelled_and_only_an_allowed_link_is_opened() {
        let opened = RefCell::new(Vec::new());
        let open = |url: &url::Url| {
            opened.borrow_mut().push(url.to_string());
            Ok(())
        };
        let response = new_window("https://github.com/example/commit/abc1234", open);
        assert_eq!(response, NewWindow::Cancel);
        let response = new_window("file:///C:/Windows/System32/calc.exe", open);
        assert_eq!(response, NewWindow::Cancel);
        let failing = |_: &url::Url| Err(DesktopError::message("open_link", "shell refused"));
        let response = new_window("https://example.test/", failing);
        assert_eq!(response, NewWindow::Cancel, "a shell failure still cancels");
        assert_eq!(*opened.borrow(), ["https://github.com/example/commit/abc1234"]);
    }
}
