// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Confine delegated tool storage to an explicit directory supplied by its storage owner.
use super::{EnvironmentOverride, LaunchEnvironment, LaunchEnvironmentError};
use std::path::Path;

impl LaunchEnvironment {
    /// Redirect home, temporary files and tool caches without mutating the host environment.
    ///
    /// The caller retains an existing writable directory through child retirement. Setup uses
    /// a disposable runtime grant; an installation-owned backend uses its verified directory
    /// under `user/comfy-data`. uv/pip caches and Python user-site/bytecode writes are disabled. Inherited
    /// WebView data-folder and browser-argument overrides cannot redirect native storage.
    /// pip and npm logs stay in the supplied directory even when inherited settings name
    /// account folders. Network proxy, certificate and executable search settings retain
    /// their captured values.
    /// # Errors
    /// Rejects invalid paths or snapshots and native environment bounds before publication.
    pub fn for_local_storage(
        snapshot: &[u16],
        directory: &Path,
    ) -> Result<Self, LaunchEnvironmentError> {
        let path = super::backend::operational(directory)?;
        let pip_log = super::backend::operational(&directory.join("pip.log"))?;
        let settings = [
            ("TEMP", None),
            ("TMP", None),
            ("TMPDIR", None),
            ("HOME", None),
            ("USERPROFILE", None),
            ("APPDATA", None),
            ("LOCALAPPDATA", None),
            ("XDG_CACHE_HOME", None),
            ("XDG_CONFIG_HOME", None),
            ("XDG_DATA_HOME", None),
            ("XDG_STATE_HOME", None),
            ("UV_CACHE_DIR", None),
            ("UV_PYTHON_INSTALL_DIR", None),
            ("UV_PYTHON_CACHE_DIR", None),
            ("PIP_CACHE_DIR", None),
            ("npm_config_cache", None),
            ("npm_config_logs_dir", None),
            ("PYTHONUSERBASE", None),
            ("PYTHONPYCACHEPREFIX", None),
            ("HF_HOME", None),
            ("HF_HUB_CACHE", None),
            ("HUGGINGFACE_HUB_CACHE", None),
            ("HF_ASSETS_CACHE", None),
            ("HF_XET_CACHE", None),
            ("TRANSFORMERS_CACHE", None),
            ("TORCH_HOME", None),
            ("TORCHINDUCTOR_CACHE_DIR", None),
            ("TRITON_CACHE_DIR", None),
            ("NUMBA_CACHE_DIR", None),
            ("CUDA_CACHE_PATH", None),
            ("MPLCONFIGDIR", None),
            ("UV_NO_CACHE", Some("1")),
            ("PIP_NO_CACHE_DIR", Some("1")),
            ("PYTHONDONTWRITEBYTECODE", Some("1")),
            ("PYTHONNOUSERSITE", Some("1")),
        ];
        let names = settings.map(|(name, _)| name.encode_utf16().collect::<Vec<_>>());
        let values =
            settings.map(|(_, value)| value.map(|text| text.encode_utf16().collect::<Vec<_>>()));
        let mut changes = names
            .iter()
            .zip(&values)
            .map(|(name, value)| EnvironmentOverride::Set {
                name,
                value: value.as_deref().unwrap_or(&path),
            })
            .collect::<Vec<_>>();
        let pip_log_name = "PIP_LOG".encode_utf16().collect::<Vec<_>>();
        changes.push(EnvironmentOverride::Set { name: &pip_log_name, value: &pip_log });
        let browser_names = removed_browser_overrides(snapshot)
            .iter()
            .map(|name| name.encode_utf16().collect::<Vec<_>>())
            .collect::<Vec<_>>();
        changes.extend(browser_names.iter().map(|name| EnvironmentOverride::Remove { name }));
        Self::from_snapshot_utf16(snapshot, &changes)
    }
}

/// The inherited WebView2 overrides a child's environment drops, so they cannot move the
/// browser profile or change how the browser runs.
#[cfg_attr(not(feature = "page-automation-qualification"), expect(unused_variables))]
pub(super) fn removed_browser_overrides(snapshot: &[u16]) -> &'static [&'static str] {
    #[cfg(feature = "page-automation-qualification")]
    if debugging_port_only(snapshot) {
        return &["WEBVIEW2_USER_DATA_FOLDER"];
    }
    &["WEBVIEW2_USER_DATA_FOLDER", "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"]
}

/// Qualification builds only: a proof drives and reads the embedded page over a loopback
/// DevTools port, so an inherited browser argument that is exactly one remote-debugging port
/// reaches the desktop child. Any other value is removed, as it always is in a shipped build.
#[cfg(feature = "page-automation-qualification")]
fn debugging_port_only(snapshot: &[u16]) -> bool {
    String::from_utf16_lossy(snapshot).split('\0').any(|entry| {
        entry.split_once('=').is_some_and(|(name, value)| {
            name.eq_ignore_ascii_case("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS")
                && value.strip_prefix("--remote-debugging-port=").is_some_and(|port| {
                    (1..=5).contains(&port.len()) && port.bytes().all(|byte| byte.is_ascii_digit())
                })
        })
    })
}

#[cfg(all(test, feature = "page-automation-qualification"))]
mod qualification_tests {
    use super::debugging_port_only;

    fn block(entries: &[&str]) -> Vec<u16> {
        let mut units = Vec::new();
        for entry in entries {
            units.extend(entry.encode_utf16());
            units.push(0);
        }
        units.push(0);
        units
    }

    #[test]
    fn only_a_single_debugging_port_reaches_the_page_in_a_qualification_build() {
        assert!(debugging_port_only(&block(&[
            r"PATH=C:\Windows",
            "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333",
        ])));
        for refused in [
            r"WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--log-file=C:\outside.log",
            "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333 --log-file=x",
            "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=",
            "OTHER=--remote-debugging-port=9333",
        ] {
            assert!(!debugging_port_only(&block(&[refused])), "{refused}");
        }
    }
}
