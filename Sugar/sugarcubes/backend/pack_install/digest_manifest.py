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
"""Record the digest of every tracked file, so local edits are found offline.

`.tracking-digests.json` sits beside `.tracking` and is written whenever this
format installs a pack. It binds itself to the exact `.tracking` bytes and the
installed version: if anything else (such as ComfyUI Manager) reinstalls the pack,
the record no longer matches and is ignored rather than trusted. Format (schema 1):
`{"schema": 1, "version": "x.y.z", "tracking": "<sha256 of .tracking>",
"files": {"<tracked file path>": "<sha256>"}}`, with exactly one digest per tracked
file. Like `.tracking`, it is not itself a tracked file.
"""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Mapping
from pathlib import Path

from .entry_paths import DIGEST_MANIFEST_NAME, TRACKING_FILE_NAME, EntryName
from .filesystem import durable_replace_text

DIGEST_RECORD_VERSION = 1
_MAX_BYTES = 8 * 1024 * 1024
_SHA256 = re.compile(r"^[0-9a-f]{64}$")


def tracking_digest(folder: Path) -> str | None:
    """Return the SHA-256 of the folder's `.tracking` bytes, if it has one."""

    record = folder / TRACKING_FILE_NAME
    if not record.is_file():
        return None
    return hashlib.sha256(record.read_bytes()).hexdigest()


def render_digest_manifest(
    *, version: str, tracking_sha256: str, files: Mapping[str, str]
) -> str:
    """Return the manifest text for one installed release."""

    return json.dumps(
        {
            "schema": DIGEST_RECORD_VERSION,
            "version": version,
            "tracking": tracking_sha256,
            "files": dict(sorted(files.items())),
        },
        indent=2,
    )


def parse_digest_manifest(
    content: bytes,
    *,
    version: str,
    tracking_sha256: str,
    entries: tuple[EntryName, ...],
) -> dict[str, str] | None:
    """Return the recorded digests, or `None` when the record does not apply.

    A record applies only to the `.tracking` bytes and version it names and must
    list exactly the tracked files; anything else means another installer changed
    the pack since, and the caller falls back to another proof.
    """

    if len(content) > _MAX_BYTES:
        return None
    try:
        payload = json.loads(content.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        return None
    if (
        not isinstance(payload, dict)
        or payload.get("schema") != DIGEST_RECORD_VERSION
        or payload.get("version") != version
        or payload.get("tracking") != tracking_sha256
    ):
        return None
    files = payload.get("files")
    if not isinstance(files, dict):
        return None
    tracked = {entry.path for entry in entries if not entry.is_directory}
    if set(files) != tracked or not all(
        isinstance(digest, str) and _SHA256.fullmatch(digest)
        for digest in files.values()
    ):
        return None
    return {str(path): str(digest) for path, digest in files.items()}


def read_digest_manifest(
    folder: Path, *, version: str, entries: tuple[EntryName, ...]
) -> dict[str, str] | None:
    """Return the folder's applicable recorded digests, or `None`."""

    record = folder / DIGEST_MANIFEST_NAME
    tracking_sha256 = tracking_digest(folder)
    if tracking_sha256 is None or not record.is_file() or record.is_symlink():
        return None
    try:
        with record.open("rb") as handle:
            content = handle.read(_MAX_BYTES + 1)
    except OSError:
        return None
    return parse_digest_manifest(
        content, version=version, tracking_sha256=tracking_sha256, entries=entries
    )


def write_digest_manifest(
    folder: Path, *, version: str, files: Mapping[str, str]
) -> None:
    """Durably record the digests of a pack whose `.tracking` is already written."""

    tracking_sha256 = tracking_digest(folder)
    if tracking_sha256 is None:
        raise FileNotFoundError("The ownership record must be written first.")
    durable_replace_text(
        folder / DIGEST_MANIFEST_NAME,
        render_digest_manifest(
            version=version, tracking_sha256=tracking_sha256, files=files
        ),
    )


__all__ = [
    "DIGEST_MANIFEST_NAME",
    "DIGEST_RECORD_VERSION",
    "parse_digest_manifest",
    "read_digest_manifest",
    "render_digest_manifest",
    "tracking_digest",
    "write_digest_manifest",
]
