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
"""Load where SugarCubes looks for its own releases.

The release locations are configuration shipped in the pack
(`release_source.json`), not URLs derived by guesswork, so the release workflow
and the updater agree on one written description. The GitHub fallback reads a
CI-built, registry-layout zip attached to a per-component release tag; it never
uses a tag's source archive, which in the monorepo is the whole repository.
"""

from __future__ import annotations

import json
import os
import re
import urllib.parse
from dataclasses import dataclass
from pathlib import Path

from .http_client import origin_is_allowed

_CONFIG_PATH = Path(__file__).with_name("release_source.json")
RELEASE_SOURCE_OVERRIDE = "SUGARCUBES_RELEASE_SOURCE_FILE"
_REPOSITORY = re.compile(r"^[A-Za-z0-9_.-]{1,100}/[A-Za-z0-9_.-]{1,100}$")
_HOST = re.compile(r"^[a-z0-9.-]{1,253}$")
_TAG_PREFIX = re.compile(r"^[A-Za-z0-9_.-]{1,64}$")
_ASSET_NAME = re.compile(r"^[A-Za-z0-9_.-]{0,100}\{version\}[A-Za-z0-9_.-]{0,100}$")
_PACK_ID = re.compile(r"^[A-Za-z0-9._-]{1,100}$")


class ReleaseConfigError(ValueError):
    """Report a release-source configuration that fails validation."""


@dataclass(frozen=True, slots=True)
class RegistrySourceConfig:
    """Locate the Comfy Registry API and the only host its archives may come from."""

    api_root: str
    archive_host: str


@dataclass(frozen=True, slots=True)
class GitHubReleaseConfig:
    """Locate the per-component GitHub release asset the fallback installs."""

    api_root: str
    repository: str
    tag_prefix: str
    asset_name: str
    download_hosts: frozenset[str]

    def tag_for(self, version: str) -> str:
        """Return the release tag of one version."""

        return f"{self.tag_prefix}{version}"

    def asset_for(self, version: str) -> str:
        """Return the registry-layout asset name of one version."""

        return self.asset_name.replace("{version}", version)


@dataclass(frozen=True, slots=True)
class ReleaseSourceConfig:
    """Bundle every place SugarCubes may fetch its own releases from."""

    pack_id: str
    registry: RegistrySourceConfig
    github_release: GitHubReleaseConfig


def configured_release_source_path() -> Path:
    """Return the shipped configuration, or the file a test setup names.

    `SUGARCUBES_RELEASE_SOURCE_FILE` lets an isolated test ComfyUI point the
    updater at a local release server without editing the pack. The named file
    passes the same validation, so it can only name allowed origins.
    """

    override = os.environ.get(RELEASE_SOURCE_OVERRIDE, "").strip()
    return Path(override) if override else _CONFIG_PATH


def load_release_config(path: Path = _CONFIG_PATH) -> ReleaseSourceConfig:
    """Load and validate one release-source configuration."""

    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ReleaseConfigError(
            "The release-source configuration is unreadable."
        ) from exc
    if not isinstance(payload, dict) or payload.get("schema") != 1:
        raise ReleaseConfigError(
            "The release-source configuration has an unknown schema."
        )
    registry = _table(payload, "registry")
    github = _table(payload, "githubRelease")
    hosts = github.get("downloadHosts")
    if not isinstance(hosts, list) or not hosts:
        raise ReleaseConfigError("githubRelease.downloadHosts must list hosts.")
    return ReleaseSourceConfig(
        pack_id=_matching(payload, "packId", _PACK_ID),
        registry=RegistrySourceConfig(
            api_root=_https_root(registry, "apiRoot"),
            archive_host=_matching(registry, "archiveHost", _HOST),
        ),
        github_release=GitHubReleaseConfig(
            api_root=_https_root(github, "apiRoot"),
            repository=_matching(github, "repository", _REPOSITORY),
            tag_prefix=_matching(github, "tagPrefix", _TAG_PREFIX),
            asset_name=_matching(github, "assetName", _ASSET_NAME),
            download_hosts=frozenset(_host(value) for value in hosts),
        ),
    )


def _table(payload: dict[str, object], key: str) -> dict[str, object]:
    """Return one nested table."""

    value = payload.get(key)
    if not isinstance(value, dict):
        raise ReleaseConfigError(f"{key} must be a table.")
    return value


def _matching(table: dict[str, object], key: str, pattern: re.Pattern[str]) -> str:
    """Return one text field matching its pattern."""

    value = table.get(key)
    if not isinstance(value, str) or not pattern.fullmatch(value):
        raise ReleaseConfigError(f"{key} is invalid.")
    return value


def _host(value: object) -> str:
    """Return one lowercase host name."""

    if not isinstance(value, str) or not _HOST.fullmatch(value):
        raise ReleaseConfigError("A download host is invalid.")
    return value


def _https_root(table: dict[str, object], key: str) -> str:
    """Return one allowed origin without path, credentials, query or fragment."""

    value = table.get(key)
    if not isinstance(value, str):
        raise ReleaseConfigError(f"{key} is invalid.")
    parsed = urllib.parse.urlsplit(value)
    try:
        port = parsed.port
    except ValueError as exc:
        raise ReleaseConfigError(f"{key} has a bad port.") from exc
    if (
        not parsed.hostname
        or not origin_is_allowed(parsed.scheme, parsed.hostname, port)
        or parsed.username is not None
        or parsed.path not in {"", "/"}
        or parsed.query
        or parsed.fragment
    ):
        raise ReleaseConfigError(f"{key} must be an HTTPS origin.")
    host = f"[{parsed.hostname}]" if ":" in parsed.hostname else parsed.hostname
    return f"{parsed.scheme}://{host}" + (f":{port}" if port is not None else "")


__all__ = [
    "GitHubReleaseConfig",
    "RegistrySourceConfig",
    "ReleaseConfigError",
    "RELEASE_SOURCE_OVERRIDE",
    "ReleaseSourceConfig",
    "configured_release_source_path",
    "load_release_config",
]
