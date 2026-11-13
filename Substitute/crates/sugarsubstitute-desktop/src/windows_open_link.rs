// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hand a release-notes link to the user's default browser through the shell.
use crate::DesktopError;
use windows::{
    Win32::UI::{
        Shell::{SEE_MASK_ASYNCOK, SEE_MASK_FLAG_NO_UI, SHELLEXECUTEINFOW, ShellExecuteExW},
        WindowsAndMessaging::SW_SHOWNORMAL,
    },
    core::{HSTRING, w},
};

/// The text handed to the shell for `url`: only plain `https` links with a host and no
/// embedded credentials, so a link can never start a program or reach a local file.
/// # Errors
/// Rejects any other link.
pub(crate) fn target(url: &url::Url) -> Result<HSTRING, DesktopError> {
    if url.scheme() != "https"
        || url.host_str().is_none_or(str::is_empty)
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(DesktopError::message("open_link", "only plain https links are opened"));
    }
    Ok(HSTRING::from(url.as_str()))
}

/// Ask the shell to open `url` in the default browser. The shell returns without waiting for
/// the browser, so the calling event-loop turn stays short.
/// # Errors
/// Rejects links [`target`] refuses and preserves the shell's failure.
pub(crate) fn open(url: &url::Url) -> Result<(), DesktopError> {
    let file = target(url)?;
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

    #[test]
    fn only_plain_https_links_reach_the_shell() -> Result<(), Box<dyn std::error::Error>> {
        let accepted = url::Url::parse("https://github.com/example/releases/tag/v2.4.1")?;
        assert_eq!(target(&accepted)?.to_string(), accepted.as_str());
        for refused in [
            "http://github.com/example/releases",
            "file:///C:/Windows/System32/calc.exe",
            "ms-settings:privacy",
            "https://user:secret@github.com/example",
            "https://user@github.com/example",
            "javascript:alert(1)",
        ] {
            assert!(target(&url::Url::parse(refused)?).is_err(), "{refused}");
        }
        Ok(())
    }
}
