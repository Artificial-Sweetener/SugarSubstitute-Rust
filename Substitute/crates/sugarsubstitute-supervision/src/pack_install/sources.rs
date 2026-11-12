// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read SugarCubes' releases from Comfy Registry and GitHub release replies.
//!
//! The Registry is authoritative. It publishes no digest, so its archives are accepted only
//! from its fixed CDN host, and only for releases it still offers: Active or Flagged (the
//! maintainer treats a scanner flag like Active); every other status is skipped. The GitHub
//! fallback reads the registry-layout asset of a per-component release and requires the
//! SHA-256 GitHub records for it. Locations come from the pack's own `release_source.json`,
//! the same description SugarCubes' updater and release workflow use.

use super::identity::ReleaseVersion;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fmt;

/// The largest release archive accepted.
pub const MAX_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024;
const INSTALLABLE_REGISTRY_STATUSES: [&str; 2] =
    ["NodeVersionStatusActive", "NodeVersionStatusFlagged"];

/// Where a release archive comes from; spelled as in the shared vectors.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReleaseSourceKind {
    /// The Comfy Registry.
    Registry,
    /// A GitHub release asset.
    GitHubRelease,
}

impl ReleaseSourceKind {
    /// Return the shared spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Registry => "registry",
            Self::GitHubRelease => "github-release",
        }
    }
}

/// Release metadata that fails validation; nothing is installed from it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetadataError(pub &'static str);

impl fmt::Display for MetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for MetadataError {}

/// Every place SugarCubes may be fetched from, as its `release_source.json` declares.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseSourceConfig {
    /// The Registry pack id, which is also the `[project] name`.
    pub pack_id: String,
    /// The Registry API origin.
    pub registry_api_root: String,
    /// The only host Registry archives may come from.
    pub registry_archive_host: String,
    /// The GitHub API origin.
    pub github_api_root: String,
    /// `owner/name` of the repository.
    pub github_repository: String,
    /// The tag prefix of SugarCubes releases.
    pub tag_prefix: String,
    /// The asset name, with `{version}` for the version.
    pub asset_name: String,
    /// The hosts GitHub downloads may come from.
    pub download_hosts: BTreeSet<String>,
}

impl ReleaseSourceConfig {
    /// Parse and validate `release_source.json` schema 1.
    ///
    /// # Errors
    /// Refuses an unknown schema, a non-HTTPS origin (plain HTTP only for loopback, which
    /// isolated test setups use) and malformed names.
    pub fn parse(bytes: &[u8]) -> Result<Self, MetadataError> {
        let invalid = MetadataError("The release-source configuration is invalid.");
        let payload: Value = serde_json::from_slice(bytes).map_err(|_| invalid.clone())?;
        if payload.get("schema").and_then(Value::as_u64) != Some(1) {
            return Err(invalid);
        }
        let text = |table: &str, key: &str| {
            let value =
                if table.is_empty() { payload.get(key) } else { payload.get(table)?.get(key) };
            value.and_then(Value::as_str).map(str::to_owned)
        };
        let origin = |table: &str| text(table, "apiRoot").filter(|root| allowed_origin(root));
        let hosts: Option<BTreeSet<String>> = payload
            .get("githubRelease")
            .and_then(|table| table.get("downloadHosts"))
            .and_then(Value::as_array)
            .and_then(|hosts| {
                hosts
                    .iter()
                    .map(|host| host.as_str().filter(|host| plain_host(host)).map(str::to_owned))
                    .collect()
            });
        let config = (|| {
            Some(Self {
                pack_id: text("", "packId").filter(|id| plain_name(id))?,
                registry_api_root: origin("registry")?,
                registry_archive_host: text("registry", "archiveHost")
                    .filter(|host| plain_host(host))?,
                github_api_root: origin("githubRelease")?,
                github_repository: text("githubRelease", "repository").filter(|name| {
                    name.split_once('/')
                        .is_some_and(|(owner, repo)| plain_name(owner) && plain_name(repo))
                })?,
                tag_prefix: text("githubRelease", "tagPrefix")
                    .filter(|prefix| plain_name(prefix))?,
                asset_name: text("githubRelease", "assetName").filter(|name| {
                    name.matches("{version}").count() == 1
                        && plain_name(&name.replace("{version}", ""))
                })?,
                download_hosts: hosts.filter(|hosts| !hosts.is_empty())?,
            })
        })();
        config.ok_or(invalid)
    }

    /// The Registry's full version list.
    pub fn registry_versions_url(&self) -> String {
        format!("{}/nodes/{}/versions", self.registry_api_root, self.pack_id)
    }

    /// The Registry's record of one exact version.
    pub fn registry_install_url(&self, version: ReleaseVersion) -> String {
        format!("{}/nodes/{}/install?version={version}", self.registry_api_root, self.pack_id)
    }

    /// The repository's release list.
    pub fn github_releases_url(&self) -> String {
        format!("{}/repos/{}/releases?per_page=50", self.github_api_root, self.github_repository)
    }

    /// The repository's release of one exact version.
    pub fn github_release_url(&self, version: ReleaseVersion) -> String {
        format!(
            "{}/repos/{}/releases/tags/{}{version}",
            self.github_api_root, self.github_repository, self.tag_prefix
        )
    }

    fn asset_for(&self, version: ReleaseVersion) -> String {
        self.asset_name.replace("{version}", &version.to_string())
    }
}

fn plain_name(text: &str) -> bool {
    (1..=100).contains(&text.len())
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
}

fn plain_host(text: &str) -> bool {
    (1..=253).contains(&text.len())
        && text.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
        })
}

fn allowed_origin(root: &str) -> bool {
    match url::Url::parse(root) {
        Ok(url) => {
            let plain = url.username().is_empty()
                && url.password().is_none()
                && matches!(url.path(), "" | "/")
                && url.query().is_none()
                && url.fragment().is_none()
                && !root.ends_with('/');
            let loopback = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
            plain && (url.scheme() == "https" || (url.scheme() == "http" && loopback))
        }
        Err(_) => false,
    }
}

/// One downloadable release archive and how to trust it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseOffer {
    /// The release version.
    pub version: ReleaseVersion,
    /// Where it comes from.
    pub source: ReleaseSourceKind,
    /// The archive URL.
    pub archive_url: String,
    /// The hosts the download may use.
    pub allowed_hosts: BTreeSet<String>,
    /// The SHA-256 the source publishes, when it publishes one.
    pub sha256: Option<String>,
}

fn url_host(url: &str) -> Option<(String, String)> {
    let parsed = url::Url::parse(url).ok()?;
    Some((parsed.host_str()?.to_owned(), parsed.path().to_owned()))
}

/// Return the newest installable release in the Registry's full version list.
///
/// # Errors
/// Refuses a reply that is not a list, names another pack or points off the CDN.
pub fn registry_latest(
    config: &ReleaseSourceConfig,
    payload: &Value,
) -> Result<Option<ReleaseOffer>, MetadataError> {
    let list =
        payload.as_array().ok_or(MetadataError("The Registry version list is not a list."))?;
    let mut newest: Option<ReleaseOffer> = None;
    for item in list {
        if let Some(offer) = registry_offer(config, item, None, true)? {
            if newest.as_ref().is_none_or(|best| offer.version > best.version) {
                newest = Some(offer);
            }
        }
    }
    Ok(newest)
}

/// Return the Registry's offer of one exact version; removed releases count only when not
/// installing, since comparing files with a published release installs nothing from it.
///
/// # Errors
/// Refuses a record for another pack or version, or with an off-CDN archive.
pub fn registry_exact(
    config: &ReleaseSourceConfig,
    payload: &Value,
    version: ReleaseVersion,
    installing: bool,
) -> Result<Option<ReleaseOffer>, MetadataError> {
    registry_offer(config, payload, Some(version), installing)
}

fn registry_offer(
    config: &ReleaseSourceConfig,
    item: &Value,
    expected: Option<ReleaseVersion>,
    installing: bool,
) -> Result<Option<ReleaseOffer>, MetadataError> {
    let record =
        item.as_object().ok_or(MetadataError("A Registry version record is not an object."))?;
    let node = record.get("node_id").and_then(Value::as_str).unwrap_or_default();
    if super::entry::fold(node) != super::entry::fold(&config.pack_id) {
        return Err(MetadataError("The Registry reply names another pack."));
    }
    let observed = record
        .get("version")
        .and_then(Value::as_str)
        .and_then(|text| ReleaseVersion::parse(text).ok());
    let observed = match (observed, expected) {
        (Some(observed), Some(expected)) if observed != expected => {
            return Err(MetadataError("The Registry reply names another version."));
        }
        (Some(observed), _) => observed,
        (None, Some(_)) => return Err(MetadataError("The Registry reply has no plain version.")),
        (None, None) => return Ok(None),
    };
    let status = record.get("status").and_then(Value::as_str).unwrap_or_default();
    if installing && !INSTALLABLE_REGISTRY_STATUSES.contains(&status) {
        return Ok(None);
    }
    let Some(archive_url) =
        record.get("downloadUrl").and_then(Value::as_str).filter(|url| !url.is_empty())
    else {
        return Ok(None);
    };
    match url_host(archive_url) {
        Some((host, path))
            if host == config.registry_archive_host && path.ends_with("/node.zip") => {}
        _ => return Err(MetadataError("The Registry archive is not on its CDN.")),
    }
    Ok(Some(ReleaseOffer {
        version: observed,
        source: ReleaseSourceKind::Registry,
        archive_url: archive_url.to_owned(),
        allowed_hosts: BTreeSet::from([config.registry_archive_host.clone()]),
        sha256: None,
    }))
}

/// Return the newest published, non-prerelease SugarCubes release on GitHub.
///
/// # Errors
/// Refuses a reply that is not a list, or a newest release whose asset is malformed.
pub fn github_latest(
    config: &ReleaseSourceConfig,
    payload: &Value,
) -> Result<Option<ReleaseOffer>, MetadataError> {
    let list = payload.as_array().ok_or(MetadataError("The GitHub reply is not a list."))?;
    let flag =
        |release: &Value, key: &str| release.get(key).and_then(Value::as_bool).unwrap_or(false);
    let newest = list
        .iter()
        .filter(|release| {
            release.is_object() && !flag(release, "draft") && !flag(release, "prerelease")
        })
        .filter_map(|release| Some((tag_version(config, release)?, release)))
        .max_by_key(|(version, _)| *version);
    match newest {
        Some((version, release)) => github_offer(config, version, release),
        None => Ok(None),
    }
}

/// Return the GitHub asset of one exact release, or `None` when it is absent or a draft.
///
/// # Errors
/// Refuses a malformed asset.
pub fn github_exact(
    config: &ReleaseSourceConfig,
    payload: &Value,
    version: ReleaseVersion,
) -> Result<Option<ReleaseOffer>, MetadataError> {
    if !payload.is_object() || payload.get("draft").and_then(Value::as_bool).unwrap_or(false) {
        return Ok(None);
    }
    github_offer(config, version, payload)
}

fn tag_version(config: &ReleaseSourceConfig, release: &Value) -> Option<ReleaseVersion> {
    let tag = release.get("tag_name")?.as_str()?;
    ReleaseVersion::parse(tag.strip_prefix(&config.tag_prefix)?).ok()
}

fn github_offer(
    config: &ReleaseSourceConfig,
    version: ReleaseVersion,
    release: &Value,
) -> Result<Option<ReleaseOffer>, MetadataError> {
    let name = config.asset_for(version);
    let asset = release.get("assets").and_then(Value::as_array).and_then(|assets| {
        assets.iter().find(|asset| asset.get("name").and_then(Value::as_str) == Some(name.as_str()))
    });
    let Some(asset) =
        asset.filter(|asset| asset.get("state").and_then(Value::as_str) == Some("uploaded"))
    else {
        return Ok(None);
    };
    let size = asset.get("size").and_then(Value::as_u64).unwrap_or(0);
    if !(1..=MAX_ARCHIVE_BYTES).contains(&size) {
        return Err(MetadataError("The GitHub asset size is out of bounds."));
    }
    let sha256 = asset
        .get("digest")
        .and_then(Value::as_str)
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .filter(|digest| super::digest_record::is_sha256(digest))
        .ok_or(MetadataError("The GitHub asset has no SHA-256 digest."))?;
    let url = asset.get("browser_download_url").and_then(Value::as_str).unwrap_or_default();
    match url_host(url) {
        Some((host, _)) if config.download_hosts.contains(&host) => {}
        _ => return Err(MetadataError("The GitHub asset is not on a GitHub download host.")),
    }
    Ok(Some(ReleaseOffer {
        version,
        source: ReleaseSourceKind::GitHubRelease,
        archive_url: url.to_owned(),
        allowed_hosts: config.download_hosts.clone(),
        sha256: Some(sha256.to_owned()),
    }))
}
