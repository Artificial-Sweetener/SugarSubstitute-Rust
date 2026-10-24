// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose preparation from producing-build feed bytes and locally selected installation paths.
mod activation;
pub(crate) mod admission;
mod engine;
mod entry;
mod options;
mod probe;
pub use entry::entry;
pub(crate) mod parent;
mod session;
#[cfg(test)]
pub(crate) mod test_support;
use std::{io, path::PathBuf, time::Duration};
use sugarsubstitute_supervision::update_preparation::{
    CachedConfiguration, Configuration, Distribution, Worker,
};

const EMBEDDED: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/update-distribution.json"));
const CAMPAIGN: Duration = Duration::from_secs(15 * 60);

/// Absent producing-build configuration disables automatic admission without reading paths.
/// Nonempty invalid configuration remains an error in the actual preparation worker.
pub(crate) fn configured() -> bool {
    !EMBEDDED.is_empty()
}

/// Construct an inert worker on an admitted preparation job. Empty producing-build configuration
/// disables automatic checking. The worker reads the protected local subscription; the distribution binds
/// only repository and transport authority. No runtime environment or neighboring file is read.
/// The caller owns job admission, cancellation and observed retirement through `Worker`.
/// # Errors
/// Rejects malformed configured bytes, unsupported origins, relative paths and zero idle timeout.
/// A malformed configured build is an error, never a disabled or up-to-date result.
pub fn preparation(
    installation: PathBuf,
    workspace: PathBuf,
    idle: Duration,
) -> io::Result<Option<Worker>> {
    configuration(EMBEDDED, installation, workspace, idle)?.map(Worker::new).transpose()
}

/// Reauthenticate prepared bytes using established installation authority, without HTTP.
/// Producing-build network configuration does not control offline activation.
pub(crate) fn cached_preparation(installation: PathBuf) -> io::Result<Worker> {
    let workspace = installation.join(".update-workspace");
    Worker::resume_cached(CachedConfiguration { installation, workspace })
}

/// Share the exact producing-byte boundary with off-screen qualification without runtime overrides.
fn configuration(
    bytes: &[u8],
    installation: PathBuf,
    workspace: PathBuf,
    idle: Duration,
) -> io::Result<Option<Configuration>> {
    if bytes.is_empty() {
        return Ok(None);
    }
    let (repository, policy) = Distribution::decode(bytes)?.into_parts();
    Ok(Some(Configuration { installation, workspace, repository, policy, idle }))
}

/// Run one dedicated preparation process on its native main thread, with no application window.
/// The distribution is compiled into this executable; arguments select only local paths.
/// The parent must own process containment and bound post-exit work independently of app leases.
/// This role persists prepared evidence then retires; it never activates a generation.
pub(crate) fn run(
    arguments: impl IntoIterator<Item = std::ffi::OsString>,
) -> Result<(), Box<dyn std::error::Error>> {
    let options = options::Options::parse(arguments)?;
    let mut session = session::Session::new(
        1024 * 1024,
        Box::new(move |_| {
            preparation(options.installation, options.workspace, Duration::from_secs(30))
        }),
    );
    sugarsubstitute_desktop::run_headless(&mut session)?;
    tracing::info!(operation="update_preparation_retired",result=?session.result,"Update preparation process retired");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    type Result = std::result::Result<(), Box<dyn std::error::Error>>;

    fn paths() -> (PathBuf, PathBuf) {
        (
            std::env::temp_dir().join("update-factory-installation"),
            std::env::temp_dir().join("update-factory-workspace"),
        )
    }

    #[test]
    fn disabled_build_constructs_no_worker_and_reads_no_installation() -> Result {
        let (installation, workspace) = paths();
        assert!(configuration(b"", installation, workspace, Duration::from_secs(30))?.is_none());
        Ok(())
    }

    #[test]
    fn compiled_policy_uses_the_actual_preparation_owner_without_reading_paths() -> Result {
        let bytes = serde_json::to_vec(&serde_json::json!({
            "schema_version":1, "repository":vec![92;32], "feed":{
                "transport":"https", "base":"https://updates.example.test/", "redirects":[]
            }
        }))?;
        {
            let (installation, workspace) = paths();
            let config = configuration(&bytes, installation, workspace, Duration::from_secs(30))?
                .ok_or("Configured feed missing")?;
            assert_eq!(config.repository, [92; 32]);
            assert_eq!(config.policy.asset("timestamp.json")?.host()?, "updates.example.test");
            let worker = Worker::new(config)?;
            assert!(!worker.pending());
            assert_eq!(
                worker.progress(),
                sugarsubstitute_supervision::update_preparation::Progress::Opening
            );
        }
        Ok(())
    }

    #[test]
    fn offline_startup_constructs_without_feed_bytes_or_network_policy() -> Result {
        let (installation, _) = paths();
        let worker = cached_preparation(installation)?;
        assert!(!worker.pending());
        assert_eq!(
            worker.progress(),
            sugarsubstitute_supervision::update_preparation::Progress::Opening
        );
        Ok(())
    }

    #[test]
    fn malformed_configured_build_is_not_treated_as_disabled() {
        let (installation, workspace) = paths();
        assert!(configuration(b"{}", installation, workspace, Duration::from_secs(30)).is_err());
    }
}
