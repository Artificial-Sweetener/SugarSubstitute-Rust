#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
#
#    This program is distributed in the hope that it will be useful,
#    but WITHOUT ANY WARRANTY; without even the implied warranty of
#    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
#    GNU Affero General Public License for more details.
#
#    You should have received a copy of the GNU Affero General Public License
#    along with this program.  If not, see <https://www.gnu.org/licenses/>.
"""Find SugarCubes releases on the Comfy Registry and on GitHub releases.

The Registry is authoritative and comes first. It publishes no digest, so its
archives are accepted only from its fixed CDN host over verified TLS, and only
for releases it still offers (active or flagged). The GitHub fallback reads the registry-layout asset
of a per-component release and requires the SHA-256 digest GitHub records for it.
"""

from __future__ import annotations

import re
import urllib.parse
from dataclasses import dataclass
from enum import Enum

from ..pack_install.errors import PackInstallError
from ..pack_install.pack_identity import ReleaseVersion, parse_release_version
from .http_client import HttpClient, HttpFailure, HttpFailureKind
from .release_config import GitHubReleaseConfig, RegistrySourceConfig

MAX_ARCHIVE_BYTES = 64 * 1024 * 1024
_MAX_METADATA_BYTES = 1024 * 1024
# Flagged means the Registry's scanner raised a concern, not that the release was
# withdrawn; the maintainer treats it like Active. Deleted, Banned and any other
# status mean the release is not offered.
INSTALLABLE_REGISTRY_STATUSES = frozenset(
    {"NodeVersionStatusActive", "NodeVersionStatusFlagged"}
)
_SHA256_DIGEST = re.compile(r"^sha256:([0-9a-f]{64})$")


class ReleaseSourceKind(str, Enum):
    """Name where a release archive comes from."""

    REGISTRY = "registry"
    GITHUB_RELEASE = "github-release"


class ReleaseMetadataError(RuntimeError):
    """Report release metadata that fails validation; never install from it."""


@dataclass(frozen=True, slots=True)
class ReleaseOffer:
    """Describe one downloadable release archive and how to trust it."""

    version: ReleaseVersion
    source: ReleaseSourceKind
    archive_url: str
    allowed_hosts: frozenset[str]
    sha256: str | None


class RegistryReleases:
    """Read SugarCubes' releases from the Comfy Registry's full version list."""

    def __init__(
        self, *, pack_id: str, config: RegistrySourceConfig, http: HttpClient
    ) -> None:
        """Bind the client to the configured Registry and pack id."""

        self._pack_id = pack_id
        self._config = config
        self._http = http
        self._api_hosts = frozenset(
            {urllib.parse.urlsplit(config.api_root).hostname or ""}
        )
        self._node_url = (
            f"{config.api_root}/nodes/{urllib.parse.quote(pack_id, safe='')}"
        )

    def latest(self) -> ReleaseOffer | None:
        """Return the newest installable release in the full version list.

        The list is read instead of a "latest" field, which can leave out a
        version the Registry has flagged.
        """

        payload = self._get(f"{self._node_url}/versions")
        if payload is None:
            return None
        if not isinstance(payload, list):
            raise ReleaseMetadataError("The Registry version list is not a list.")
        offers = [
            offer
            for item in payload
            if (offer := self._offer(item, expected=None, installing=True)) is not None
        ]
        return max(offers, key=lambda offer: offer.version, default=None)

    def exact(
        self, version: ReleaseVersion, *, installing: bool
    ) -> ReleaseOffer | None:
        """Return one exact release, or `None` when the Registry has none.

        Installing skips a release the Registry has removed; comparing installed
        files with a published release does not, because nothing from it is
        installed.
        """

        query = urllib.parse.urlencode({"version": str(version)})
        payload = self._get(f"{self._node_url}/install?{query}")
        if payload is None:
            return None
        return self._offer(payload, expected=version, installing=installing)

    def _get(self, url: str) -> object | None:
        """Fetch one Registry reply, treating a missing page as no release."""

        try:
            return self._http.get_json(
                url, allowed_hosts=self._api_hosts, max_bytes=4 * _MAX_METADATA_BYTES
            )
        except HttpFailure as exc:
            if exc.kind is HttpFailureKind.NOT_FOUND:
                return None
            raise

    def _offer(
        self, payload: object, *, expected: ReleaseVersion | None, installing: bool
    ) -> ReleaseOffer | None:
        """Validate one version record; a record for another pack is refused."""

        if not isinstance(payload, dict):
            raise ReleaseMetadataError("A Registry version record is not an object.")
        if str(payload.get("node_id", "")).casefold() != self._pack_id.casefold():
            raise ReleaseMetadataError("The Registry reply names another pack.")
        try:
            observed = parse_release_version(payload.get("version"))
        except PackInstallError:
            if expected is not None:
                raise ReleaseMetadataError(
                    "The Registry reply has no plain version."
                ) from None
            return None
        if expected is not None and observed != expected:
            raise ReleaseMetadataError("The Registry reply names another version.")
        if installing and payload.get("status") not in INSTALLABLE_REGISTRY_STATUSES:
            return None
        archive_url = payload.get("downloadUrl")
        if not isinstance(archive_url, str) or not archive_url:
            return None
        parsed = urllib.parse.urlsplit(archive_url)
        if parsed.hostname != self._config.archive_host or not parsed.path.endswith(
            "/node.zip"
        ):
            raise ReleaseMetadataError("The Registry archive is not on its CDN.")
        return ReleaseOffer(
            version=observed,
            source=ReleaseSourceKind.REGISTRY,
            archive_url=archive_url,
            allowed_hosts=frozenset({self._config.archive_host}),
            sha256=None,
        )


class GitHubReleases:
    """Read the registry-layout release assets attached to per-component tags."""

    def __init__(self, *, config: GitHubReleaseConfig, http: HttpClient) -> None:
        """Bind the client to the configured repository and asset naming."""

        self._config = config
        self._http = http
        self._api_hosts = frozenset(
            {urllib.parse.urlsplit(config.api_root).hostname or ""}
        )

    def latest(self) -> ReleaseOffer | None:
        """Return the newest published, non-prerelease SugarCubes release."""

        payload = self._http.get_json(
            f"{self._config.api_root}/repos/{self._config.repository}/releases?per_page=50",
            allowed_hosts=self._api_hosts,
            max_bytes=4 * _MAX_METADATA_BYTES,
        )
        if not isinstance(payload, list):
            raise ReleaseMetadataError("The GitHub reply is not a list.")
        candidates: list[tuple[ReleaseVersion, dict[str, object]]] = []
        for release in payload:
            if (
                not isinstance(release, dict)
                or release.get("draft")
                or release.get("prerelease")
            ):
                continue
            version = self._tag_version(release.get("tag_name"))
            if version is not None:
                candidates.append((version, release))
        if not candidates:
            return None
        version, release = max(candidates, key=lambda candidate: candidate[0])
        return self._offer(version, release)

    def exact(self, version: ReleaseVersion) -> ReleaseOffer | None:
        """Return the asset of one exact release, or `None` when it is absent."""

        tag = urllib.parse.quote(self._config.tag_for(str(version)), safe="")
        try:
            payload = self._http.get_json(
                f"{self._config.api_root}/repos/{self._config.repository}/releases/tags/{tag}",
                allowed_hosts=self._api_hosts,
                max_bytes=_MAX_METADATA_BYTES,
            )
        except HttpFailure as exc:
            if exc.kind is HttpFailureKind.NOT_FOUND:
                return None
            raise
        if not isinstance(payload, dict) or payload.get("draft"):
            return None
        return self._offer(version, payload)

    def _tag_version(self, tag: object) -> ReleaseVersion | None:
        """Return the version a SugarCubes tag names, ignoring other components."""

        if not isinstance(tag, str) or not tag.startswith(self._config.tag_prefix):
            return None
        try:
            return parse_release_version(tag.removeprefix(self._config.tag_prefix))
        except PackInstallError:
            return None

    def _offer(
        self, version: ReleaseVersion, release: dict[str, object]
    ) -> ReleaseOffer | None:
        """Return the release's registry-layout asset after validating it."""

        assets = release.get("assets")
        expected_name = self._config.asset_for(str(version))
        asset = next(
            (
                item
                for item in (assets if isinstance(assets, list) else [])
                if isinstance(item, dict) and item.get("name") == expected_name
            ),
            None,
        )
        if asset is None or asset.get("state") != "uploaded":
            return None
        size = asset.get("size")
        digest = _SHA256_DIGEST.fullmatch(str(asset.get("digest", "")))
        url = asset.get("browser_download_url")
        if not isinstance(size, int) or not 0 < size <= MAX_ARCHIVE_BYTES:
            raise ReleaseMetadataError("The GitHub asset size is out of bounds.")
        if digest is None:
            raise ReleaseMetadataError("The GitHub asset has no SHA-256 digest.")
        if (
            not isinstance(url, str)
            or urllib.parse.urlsplit(url).hostname not in self._config.download_hosts
        ):
            raise ReleaseMetadataError(
                "The GitHub asset is not on a GitHub download host."
            )
        return ReleaseOffer(
            version=version,
            source=ReleaseSourceKind.GITHUB_RELEASE,
            archive_url=url,
            allowed_hosts=self._config.download_hosts,
            sha256=digest[1],
        )


__all__ = [
    "GitHubReleases",
    "INSTALLABLE_REGISTRY_STATUSES",
    "MAX_ARCHIVE_BYTES",
    "RegistryReleases",
    "ReleaseMetadataError",
    "ReleaseOffer",
    "ReleaseSourceKind",
]
