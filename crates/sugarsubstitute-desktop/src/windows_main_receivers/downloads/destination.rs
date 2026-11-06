// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Set and verify installation-local browser downloads before any frontend navigation.
use super::super::NativeString;
use crate::DesktopError;
use std::path::{Path, PathBuf};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2, ICoreWebView2_13, ICoreWebView2Profile,
};
use windows::core::{HSTRING, Interface, PWSTR};

/// Configure on the creating STA while the authorized profile and browser remain retained.
/// WebView2 creates the child on its first download; errors prevent frontend navigation.
pub(crate) fn configure(core: &ICoreWebView2, profile: &Path) -> Result<(), DesktopError> {
    configure_with(profile, |destination| {
        // SAFETY: This STA retains the core. Adopt every returned reference before status,
        // including partial failed outputs; the outer browser retains its profile lifetime.
        unsafe {
            let core = core
                .cast::<ICoreWebView2_13>()
                .map_err(|cause| DesktopError::new("configure_download_destination", cause))?;
            let mut raw = std::ptr::null_mut();
            let status = (core.vtable().Profile)(core.as_raw(), &mut raw);
            let profile =
                if raw.is_null() { None } else { Some(ICoreWebView2Profile::from_raw(raw)) };
            status
                .ok()
                .map_err(|cause| DesktopError::new("configure_download_destination", cause))?;
            let profile = profile.ok_or_else(|| invalid("native profile missing"))?;
            profile
                .SetDefaultDownloadFolderPath(&HSTRING::from(destination.as_os_str()))
                .map_err(|cause| DesktopError::new("configure_download_destination", cause))?;
            let mut raw = PWSTR::null();
            let status = profile.DefaultDownloadFolderPath(&mut raw);
            let observed = NativeString(raw);
            status.map_err(|cause| DesktopError::new("observe_download_destination", cause))?;
            if observed.0.is_null() {
                return Err(invalid("native destination missing"));
            }
            let mut value = String::with_capacity(crate::main_navigation::MAX_URL_BYTES);
            crate::main_events::copy_utf16(&mut value, |index| {
                observed.0.as_ptr().add(index).read()
            })?;
            Ok(PathBuf::from(value))
        }
    })
}

/// Verify the native boundary's readback rather than trusting a successful setter alone.
fn configure_with(
    profile: &Path,
    native: impl FnOnce(&Path) -> Result<PathBuf, DesktopError>,
) -> Result<(), DesktopError> {
    sugarsubstitute_contracts::configuration::LaunchConfiguration::validate_path(profile)
        .map_err(|cause| DesktopError::new("configure_download_destination", cause))?;
    let destination = profile.join("downloads");
    if native(&destination)? != destination {
        return Err(invalid("native destination changed"));
    }
    Ok(())
}

fn invalid(message: &'static str) -> DesktopError {
    DesktopError::message("configure_download_destination", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downloads_use_the_authorized_unicode_profile_before_native_admission()
    -> Result<(), DesktopError> {
        let profile = Path::new(r"E:\Sugar 糖\user\desktop-data\profile");
        configure_with(profile, |destination| {
            assert_eq!(destination, profile.join("downloads"));
            Ok(destination.to_path_buf())
        })?;
        let mut admitted = false;
        assert!(
            configure_with(Path::new("relative"), |_| {
                admitted = true;
                Err(invalid("invalid input reached native effects"))
            })
            .is_err()
        );
        assert!(!admitted);
        Ok(())
    }

    #[test]
    fn inherited_destinations_and_native_failures_block_navigation() {
        let profile = Path::new(r"E:\Sugar\user\desktop-data\profile");
        assert!(
            configure_with(profile, |_| Ok(PathBuf::from(r"C:\Users\user\Downloads"))).is_err()
        );
        assert!(configure_with(profile, |_| Err(invalid("native failure"))).is_err());
    }
}
