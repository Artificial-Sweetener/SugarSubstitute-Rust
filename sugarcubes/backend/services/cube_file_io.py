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
"""Own cube artifact filesystem reads, hashing, discovery, and cleanup."""

from __future__ import annotations

import hashlib
import json
import logging
import os
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Mapping, Optional

_logger = logging.getLogger(__name__)


def format_timestamp(timestamp: float) -> str:
    """Return an ISO 8601 timestamp for UI metadata."""

    return datetime.fromtimestamp(timestamp, tz=timezone.utc).isoformat(
        timespec="seconds"
    )


def safe_relative_path(path: Path, base: Path) -> Optional[str]:
    """Return the path relative to `base` using forward slashes."""

    try:
        relative = path.relative_to(base)
    except ValueError:
        return None
    return str(relative).replace(os.sep, "/")


def format_display_path(path: Path, extension_root: Path) -> str:
    """Return a human-friendly display path."""

    for candidate in (Path.cwd(), extension_root):
        try:
            relative = path.relative_to(candidate)
            return str(relative).replace(os.sep, "/")
        except ValueError:
            continue
    return str(path.resolve()).replace(os.sep, "/")


def apply_cube_version(payload: Mapping[str, Any], version: str) -> None:
    """Apply a suggested version to a mutable cube payload."""

    if version and isinstance(payload, dict):
        payload["version"] = version


def read_cube_payload(path: Path) -> tuple[Optional[Mapping[str, Any]], Optional[str]]:
    """Read a lightweight cube payload from disk."""

    try:
        with path.open("r", encoding="utf-8") as handle:
            payload = json.load(handle)
    except json.JSONDecodeError:
        return None, "Cube file is not valid JSON"
    except (OSError, UnicodeDecodeError) as exc:  # pragma: no cover - diagnostics only
        _logger.warning("SugarCubes: failed to read cube %s", path, exc_info=exc)
        return None, str(exc)
    if not isinstance(payload, Mapping):
        return None, "Cube root must be a JSON object"
    return dict(payload), None


def read_cube_payload_with_hash(
    path: Path,
) -> tuple[Optional[Mapping[str, Any]], Optional[str], str]:
    """Read a cube payload and content hash with one filesystem read."""

    try:
        content = path.read_bytes()
    except OSError as exc:  # pragma: no cover - diagnostics only
        _logger.warning("SugarCubes: failed to read cube %s", path, exc_info=exc)
        return None, str(exc), ""
    try:
        payload = json.loads(content.decode("utf-8"))
    except UnicodeDecodeError as exc:  # pragma: no cover - diagnostics only
        _logger.warning("SugarCubes: failed to decode cube %s", path, exc_info=exc)
        return None, str(exc), ""
    except json.JSONDecodeError:
        return None, "Cube file is not valid JSON", compute_cube_content_hash_bytes(content)
    if not isinstance(payload, Mapping):
        return None, "Cube root must be a JSON object", compute_cube_content_hash_bytes(content)
    return dict(payload), None, compute_cube_content_hash_bytes(content)


def compute_cube_content_hash(path: Path) -> str:
    """Return a stable content hash for one source-owned cube artifact."""

    return compute_cube_content_hash_bytes(path.read_bytes())


def compute_cube_content_hash_bytes(content: bytes) -> str:
    """Return a stable content hash for one cube artifact payload."""

    digest = hashlib.sha256()
    digest.update(b"sugarcube\0")
    digest.update(content)
    return f"sha256:{digest.hexdigest()}"


def list_cube_files(base_dir: Path) -> list[Path]:
    """Return all managed cube files, excluding backup folders."""

    if not base_dir.exists():
        return []
    files: list[Path] = []
    for path in base_dir.rglob("*.cube"):
        if not path.is_file():
            continue
        try:
            relative = path.relative_to(base_dir)
        except ValueError:
            relative = None
        if relative is not None:
            parts = [part.lower() for part in relative.parts if part]
            if parts and parts[0] in {"old", "backup", "_old", "_history"}:
                continue
        files.append(path.resolve())
    files.sort()
    return files


def cleanup_failed_import(path: Path) -> None:
    """Remove a failed imported cube when cleanup is still possible."""

    if not path.exists():
        return
    try:
        path.unlink()
    except OSError:
        _logger.warning(
            "SugarCubes: unable to clean up failed import %s",
            path,
            exc_info=True,
        )
