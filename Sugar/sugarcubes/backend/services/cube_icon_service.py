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
"""Normalize and resolve cube icon asset metadata."""

from __future__ import annotations

from pathlib import Path, PurePosixPath, PureWindowsPath
from typing import Any, Mapping
from urllib.parse import quote, urlsplit

SUPPORTED_ICON_MEDIA_TYPES = {
    ".png": "image/png",
    ".svg": "image/svg+xml",
}


class CubeIconError(ValueError):
    """Raise when cube icon metadata is unsafe or unsupported."""


def build_icon_asset_url(cube_id: str) -> str:
    """Build the SugarCubes route for one cube definition icon."""

    return f"/sugarcubes/assets/icon?cube_id={quote(cube_id, safe='')}"


def normalize_icon_metadata(value: Any) -> dict[str, str] | None:
    """Return normalized icon metadata or `None` when no icon is declared.

    Raises:
        CubeIconError: The caller supplied malformed icon metadata.
    """

    if value is None:
        return None
    if not isinstance(value, Mapping):
        raise CubeIconError("metadata.icon must be an object")

    kind = _read_trimmed(value.get("kind"))
    if kind != "asset":
        raise CubeIconError("metadata.icon.kind must be 'asset'")

    path = normalize_icon_asset_path(value.get("path"))
    suffix = PurePosixPath(path).suffix.lower()
    media_type = SUPPORTED_ICON_MEDIA_TYPES[suffix]
    submitted_media_type = _read_trimmed(value.get("media_type"))
    if submitted_media_type and submitted_media_type != media_type:
        raise CubeIconError(
            f"metadata.icon.media_type must be '{media_type}' for {suffix} assets"
        )

    return {
        "kind": "asset",
        "path": path,
        "media_type": media_type,
    }


def normalize_existing_icon_metadata(value: Any) -> dict[str, str] | None:
    """Normalize persisted icon metadata while ignoring invalid legacy values."""

    try:
        return normalize_icon_metadata(value)
    except CubeIconError:
        return None


def normalize_icon_asset_path(value: Any) -> str:
    """Normalize a repo-relative icon path and reject unsafe references."""

    raw = _read_trimmed(value)
    if not raw:
        raise CubeIconError("metadata.icon.path is required")
    parsed = urlsplit(raw)
    if parsed.scheme or parsed.netloc or parsed.query or parsed.fragment:
        raise CubeIconError("metadata.icon.path must be a repo-relative file path")
    if ":" in raw:
        raise CubeIconError("metadata.icon.path must not include a drive or URL scheme")

    normalized = raw.replace("\\", "/")
    windows_path = PureWindowsPath(raw)
    posix_path = PurePosixPath(normalized)
    if windows_path.is_absolute() or windows_path.drive or posix_path.is_absolute():
        raise CubeIconError("metadata.icon.path must be repo-relative")

    parts = [part for part in posix_path.parts if part not in {"", "."}]
    if not parts or any(part == ".." for part in parts):
        raise CubeIconError("metadata.icon.path must stay inside the cube repository")

    suffix = PurePosixPath(*parts).suffix.lower()
    if suffix not in SUPPORTED_ICON_MEDIA_TYPES:
        raise CubeIconError("metadata.icon.path must reference a .png or .svg file")
    return str(PurePosixPath(*parts))


def resolve_icon_asset_path(base_dir: Path, icon: Mapping[str, str]) -> Path:
    """Resolve normalized icon metadata to a file path inside the source root."""

    path = normalize_icon_asset_path(icon.get("path"))
    root = base_dir.resolve()
    resolved = (root / Path(path)).resolve()
    try:
        resolved.relative_to(root)
    except ValueError as exc:
        raise CubeIconError(
            "metadata.icon.path must stay inside the cube repository"
        ) from exc
    return resolved


def attach_icon_url(
    icon: Mapping[str, str] | None, cube_id: str
) -> dict[str, str] | None:
    """Return browser-facing icon metadata with a safe asset URL."""

    if not icon:
        return None
    return {
        "kind": icon["kind"],
        "media_type": icon["media_type"],
        "url": build_icon_asset_url(cube_id),
        "repo_relative_path": icon["path"],
    }


def _read_trimmed(value: Any) -> str:
    """Read a normalized string field."""

    if isinstance(value, str):
        return value.strip()
    return ""
