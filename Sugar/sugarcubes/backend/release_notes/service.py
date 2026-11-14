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
"""Read SugarCubes' published release notes when the page asks for them.

The notes are the changelog semantic-release committed at the running version's
release tag, read through the GitHub contents API on the release source's
configured API origin with the self-updater's bounded HTTPS client: allow-listed
host, per-read timeout, overall deadline and byte limit. Nothing is cached or
compiled in; each request reads the published changelog again.
"""

from __future__ import annotations

import base64
import binascii
import logging
import urllib.parse
from dataclasses import dataclass
from enum import Enum

from ..pack_install.errors import PackInstallError
from ..pack_install.pack_identity import ReleaseVersion, parse_release_version
from ..self_update.http_client import (
    HttpClient,
    HttpFailure,
    HttpFailureKind,
    UrllibHttpClient,
)
from ..self_update.release_config import (
    GitHubReleaseConfig,
    configured_release_source_path,
    load_release_config,
)
from .changelog import ReleaseNotes, notes_in_range

CHANGELOG_PATH = "Sugar/CHANGELOG.md"
"""Where SugarCubes' changelog lives in the release repository."""
PRODUCT = "SugarCubes"
"""The product name the page labels these notes with."""
_MAX_CONTENTS_BYTES = 4 * 1024 * 1024

_LOGGER = logging.getLogger(__name__)


class NotesFailure(str, Enum):
    """Name why no notes could be shown."""

    UNAVAILABLE = "unavailable"
    NOT_PUBLISHED = "not-published"


@dataclass(frozen=True, slots=True)
class NotesUnavailable:
    """Report one failed read with the reason the page shows."""

    reason: NotesFailure


class ReleaseNotesService:
    """Answer the page's release notes requests for the running SugarCubes."""

    def __init__(
        self, *, config: GitHubReleaseConfig, http: HttpClient, running_version: str
    ) -> None:
        """Bind the release repository, the bounded client and the running version."""

        self._config = config
        self._http = http
        self._running_version = running_version
        self._api_hosts = frozenset(
            {urllib.parse.urlsplit(config.api_root).hostname or ""}
        )

    @property
    def current_version(self) -> str:
        """Return the running SugarCubes version."""

        return self._running_version

    def notes_since(self, since: ReleaseVersion) -> ReleaseNotes | NotesUnavailable:
        """Return the published notes after `since` through the running version."""

        try:
            through = parse_release_version(self._running_version)
        except PackInstallError:
            return NotesUnavailable(NotesFailure.NOT_PUBLISHED)
        try:
            payload = self._http.get_json(
                self._contents_url(through),
                allowed_hosts=self._api_hosts,
                max_bytes=_MAX_CONTENTS_BYTES,
            )
        except HttpFailure as failure:
            if failure.kind is HttpFailureKind.NOT_FOUND:
                return NotesUnavailable(NotesFailure.NOT_PUBLISHED)
            _LOGGER.warning(
                "SugarCubes release notes could not be read",
                extra={
                    "operation": "release_notes_fetch",
                    "failure": failure.kind.value,
                },
            )
            return NotesUnavailable(NotesFailure.UNAVAILABLE)
        changelog = _decoded_contents(payload)
        if changelog is None:
            _LOGGER.warning(
                "SugarCubes release notes reply was unreadable",
                extra={"operation": "release_notes_decode"},
            )
            return NotesUnavailable(NotesFailure.UNAVAILABLE)
        notes = notes_in_range(changelog, since=since, through=through)
        return (
            notes if notes is not None else NotesUnavailable(NotesFailure.NOT_PUBLISHED)
        )

    def _contents_url(self, version: ReleaseVersion) -> str:
        """Return the contents API address of the changelog at one release tag."""

        query = urllib.parse.urlencode({"ref": self._config.tag_for(str(version))})
        path = urllib.parse.quote(CHANGELOG_PATH, safe="/")
        return f"{self._config.api_root}/repos/{self._config.repository}/contents/{path}?{query}"


def build_release_notes_service(
    *,
    running_version: str,
    config: GitHubReleaseConfig | None = None,
    http: HttpClient | None = None,
) -> ReleaseNotesService:
    """Build the notes reader over the shipped release source and real transport."""

    return ReleaseNotesService(
        config=config
        or load_release_config(configured_release_source_path()).github_release,
        http=http or UrllibHttpClient(),
        running_version=running_version,
    )


def _decoded_contents(payload: object) -> str | None:
    """Decode a contents API reply's base64 file text, or return None."""

    if not isinstance(payload, dict) or payload.get("encoding") != "base64":
        return None
    content = payload.get("content")
    if not isinstance(content, str):
        return None
    try:
        raw = base64.b64decode("".join(content.split()), validate=True)
        return raw.decode("utf-8")
    except (binascii.Error, UnicodeDecodeError):
        return None


__all__ = [
    "CHANGELOG_PATH",
    "NotesFailure",
    "NotesUnavailable",
    "PRODUCT",
    "ReleaseNotesService",
    "build_release_notes_service",
]
