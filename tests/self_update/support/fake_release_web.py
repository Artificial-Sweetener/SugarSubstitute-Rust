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
"""Fake the Comfy Registry and GitHub release endpoints at the HTTP boundary."""

from __future__ import annotations

import hashlib
import io
import zipfile
from collections.abc import Callable, Mapping
from dataclasses import dataclass, field
from pathlib import Path

from sugarcubes.backend.self_update.http_client import (
    Cancelled,
    HttpFailure,
    HttpFailureKind,
    require_allowed_url,
)
from sugarcubes.backend.self_update.release_config import load_release_config

CONFIG = load_release_config()
REGISTRY_NODE = f"{CONFIG.registry.api_root}/nodes/SugarCubes"
REGISTRY_API = f"{REGISTRY_NODE}/install"
REGISTRY_VERSIONS = f"{REGISTRY_NODE}/versions"
GITHUB_API = (
    f"{CONFIG.github_release.api_root}/repos/{CONFIG.github_release.repository}"
)
ACTIVE = "NodeVersionStatusActive"
FLAGGED = "NodeVersionStatusFlagged"


def zip_bytes(files: Mapping[str, str]) -> bytes:
    """Return a flat registry-layout archive of the given files."""

    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as archive:
        for name, text in files.items():
            archive.writestr(name, text)
    return buffer.getvalue()


@dataclass
class FakeReleaseWeb:
    """Serve canned JSON and archives, enforcing the client's host allowlist."""

    json: dict[str, object] = field(default_factory=dict)
    blobs: dict[str, bytes] = field(default_factory=dict)
    failures: dict[str, HttpFailureKind] = field(default_factory=dict)
    requests: list[str] = field(default_factory=list)

    def get_json(
        self, url: str, *, allowed_hosts: frozenset[str], max_bytes: int
    ) -> object:
        """Return the canned JSON for one URL."""

        self._enter(url, allowed_hosts)
        if url not in self.json:
            raise HttpFailure(HttpFailureKind.NOT_FOUND, "no such page")
        return self.json[url]

    def download(
        self,
        url: str,
        target: Path,
        *,
        allowed_hosts: frozenset[str],
        max_bytes: int,
        cancelled: Cancelled,
    ) -> str:
        """Write the canned archive for one URL and return its digest."""

        self._enter(url, allowed_hosts)
        content = self.blobs.get(url)
        if content is None:
            raise HttpFailure(HttpFailureKind.NOT_FOUND, "no such file")
        if len(content) > max_bytes:
            raise HttpFailure(HttpFailureKind.TOO_LARGE, "too large")
        target.write_bytes(content)
        return hashlib.sha256(content).hexdigest()

    def _enter(self, url: str, allowed_hosts: frozenset[str]) -> None:
        """Record the request, apply the real allowlist and any injected failure."""

        self.requests.append(url)
        require_allowed_url(url, allowed_hosts)
        failure = next(
            (kind for prefix, kind in self.failures.items() if url.startswith(prefix)),
            None,
        )
        if failure is not None:
            raise HttpFailure(failure, "injected")

    def publish_registry(
        self,
        version: str,
        files: Mapping[str, str],
        *,
        status: str = ACTIVE,
    ) -> str:
        """Publish one Registry release and return its archive URL."""

        url = f"https://{CONFIG.registry.archive_host}/artificialsweetener/SugarCubes/{version}/node.zip"
        descriptor = {
            "node_id": "SugarCubes",
            "version": version,
            "status": status,
            "downloadUrl": url,
        }
        self.json[f"{REGISTRY_API}?version={version}"] = descriptor
        listing = self.json.setdefault(REGISTRY_VERSIONS, [])
        assert isinstance(listing, list)
        listing.insert(0, descriptor)
        self.blobs[url] = zip_bytes(files)
        return url

    def publish_github(
        self,
        version: str,
        files: Mapping[str, str],
        *,
        digest: Callable[[bytes], str] | None = None,
    ) -> str:
        """Publish one GitHub release with its registry-layout asset."""

        content = zip_bytes(files)
        tag = CONFIG.github_release.tag_for(version)
        asset_name = CONFIG.github_release.asset_for(version)
        url = f"https://github.com/{CONFIG.github_release.repository}/releases/download/{tag}/{asset_name}"
        recorded = (
            digest(content)
            if digest
            else f"sha256:{hashlib.sha256(content).hexdigest()}"
        )
        release = {
            "tag_name": tag,
            "draft": False,
            "prerelease": False,
            "assets": [
                {
                    "name": asset_name,
                    "state": "uploaded",
                    "size": len(content),
                    "digest": recorded,
                    "browser_download_url": url,
                }
            ],
        }
        self.json[f"{GITHUB_API}/releases/tags/{tag}"] = release
        listing = self.json.setdefault(f"{GITHUB_API}/releases?per_page=50", [])
        assert isinstance(listing, list)
        listing.append(release)
        self.blobs[url] = content
        return url

    def publish_empty_registry(self) -> None:
        """Publish a Registry entry with no versions."""

        self.json.setdefault(REGISTRY_VERSIONS, [])

    def publish_empty_github(self) -> None:
        """Publish a repository with no SugarCubes releases."""

        self.json.setdefault(f"{GITHUB_API}/releases?per_page=50", [])


class InlineRunner:
    """Run each job at once, so tests observe its result deterministically."""

    def submit(self, job: Callable[[], None]) -> None:
        """Run one job now."""

        job()

    def close(self) -> None:
        """Nothing runs in the background."""


__all__ = [
    "ACTIVE",
    "CONFIG",
    "FLAGGED",
    "REGISTRY_VERSIONS",
    "FakeReleaseWeb",
    "InlineRunner",
    "zip_bytes",
]
