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
"""Canonical source-qualified SugarCube identity helpers."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import PurePosixPath
import re

_OWNER_RE = re.compile(r"^[A-Za-z0-9](?:[A-Za-z0-9-]{0,38})$")
_REPO_RE = re.compile(r"^[A-Za-z0-9._-]+$")
_LOCAL_NAMESPACE_RE = re.compile(r"^[A-Za-z0-9](?:[A-Za-z0-9._-]{0,62})$")
_WINDOWS_UNSAFE_FILENAME_RE = re.compile(r'[<>:"|?*\x00-\x1f]')
RESERVED_SOURCE_NAMES = frozenset({"local", "flavors"})


class CubeIdentityError(ValueError):
    """Raise when a cube id violates the canonical source-qualified contract."""


@dataclass(frozen=True)
class CanonicalCubeId:
    """Represent one canonical source-qualified cube identifier."""

    source_kind: str
    path: str
    owner: str = ""
    repo: str = ""
    namespace: str = ""

    @property
    def repo_ref(self) -> str:
        """Return the owning GitHub repository reference when applicable."""

        if self.source_kind != "github":
            return ""
        return f"{self.owner}/{self.repo}"

    @property
    def source_root(self) -> str:
        """Return the canonical identity prefix without the cube-relative path."""

        if self.source_kind == "github":
            return f"{self.owner}/{self.repo}"
        return f"local/{self.namespace}"

    def to_string(self) -> str:
        """Return the canonical string representation."""

        return f"{self.source_root}/{self.path}"


def parse_canonical_cube_id(value: str) -> CanonicalCubeId:
    """Parse and validate one canonical source-qualified cube identity."""

    if not isinstance(value, str):
        raise CubeIdentityError("Cube id must be a string")
    cleaned = value.strip()
    if not cleaned:
        raise CubeIdentityError("Cube id is required")

    parts = cleaned.split("/", 1)
    if len(parts) != 2:
        raise CubeIdentityError(
            "Cube id must use canonical <owner>/<repo>/<path>.cube or local/<namespace>/<path>.cube format"
        )
    first_segment, remainder = parts
    if first_segment == "local":
        return _parse_local_cube_id(remainder)
    return _parse_github_cube_id(cleaned)


def is_canonical_cube_id(value: str) -> bool:
    """Return whether a string matches the canonical identity format."""

    try:
        parse_canonical_cube_id(value)
    except CubeIdentityError:
        return False
    return True


def build_canonical_cube_id(
    *,
    source_kind: str,
    path: str,
    owner: str = "",
    repo: str = "",
    namespace: str = "",
) -> str:
    """Build one canonical cube id from validated components."""

    if source_kind == "github":
        return parse_canonical_cube_id(f"{owner}/{repo}/{path}").to_string()
    if source_kind == "local":
        return parse_canonical_cube_id(f"local/{namespace}/{path}").to_string()
    raise CubeIdentityError("Cube id source must be 'github' or 'local'")


def suggest_canonical_cube_path(default_alias: str) -> str:
    """Return the source-relative filename for one default alias."""

    trimmed = default_alias.strip() if isinstance(default_alias, str) else ""
    filename = trimmed or "cube"
    if not filename.lower().endswith(".cube"):
        filename = f"{filename}.cube"
    return _validate_cube_path_segment(filename)


def derive_cube_id_from_default_alias(cube_id: str, default_alias: str) -> str:
    """Return a cube id with the same source and parent path but alias-derived filename."""

    parsed = parse_canonical_cube_id(cube_id)
    path_parts = parsed.path.split("/")
    path_parts[-1] = suggest_canonical_cube_path(default_alias)
    return build_canonical_cube_id(
        source_kind=parsed.source_kind,
        owner=parsed.owner,
        repo=parsed.repo,
        namespace=parsed.namespace,
        path="/".join(path_parts),
    )


def derive_source_author_label(cube_id: str, *, local_label: str = "local") -> str:
    """Return the product author label derived from the canonical cube id."""

    parsed = parse_canonical_cube_id(cube_id)
    if parsed.source_kind == "github":
        return parsed.repo_ref
    return local_label


def validate_github_repo_ref(owner: str, repo: str) -> tuple[str, str]:
    """Validate one GitHub owner/repo pair."""

    parsed = parse_canonical_cube_id(f"{owner}/{repo}/placeholder.cube")
    return parsed.owner, parsed.repo


def validate_local_namespace(namespace: str) -> str:
    """Validate one canonical local namespace segment."""

    cleaned = namespace.strip() if isinstance(namespace, str) else ""
    if not cleaned:
        raise CubeIdentityError("Cube id local namespace is required")
    if cleaned.lower() in RESERVED_SOURCE_NAMES:
        raise CubeIdentityError(f"Cube id local namespace '{cleaned}' is reserved")
    if not _LOCAL_NAMESPACE_RE.fullmatch(cleaned):
        raise CubeIdentityError("Cube id local namespace is invalid")
    return cleaned


def _parse_github_cube_id(remainder: str) -> CanonicalCubeId:
    """Parse one canonical GitHub-backed cube identity."""

    parts = remainder.split("/", 2)
    if len(parts) != 3:
        raise CubeIdentityError(
            "Cube id must use canonical <owner>/<repo>/<path>.cube format"
        )
    owner, repo, relative_path = (part.strip() for part in parts)
    if owner.lower() in RESERVED_SOURCE_NAMES:
        raise CubeIdentityError(f"Cube id owner '{owner}' is reserved")
    if not _OWNER_RE.fullmatch(owner):
        raise CubeIdentityError("Cube id owner is invalid")
    if not _REPO_RE.fullmatch(repo):
        raise CubeIdentityError("Cube id repo is invalid")
    normalized_path = _normalize_cube_relative_path(relative_path)
    return CanonicalCubeId(
        source_kind="github",
        owner=owner,
        repo=repo,
        path=normalized_path,
    )


def _parse_local_cube_id(remainder: str) -> CanonicalCubeId:
    """Parse one canonical local cube identity."""

    parts = remainder.split("/", 1)
    if len(parts) != 2:
        raise CubeIdentityError(
            "Cube id must use canonical local/<namespace>/<path>.cube format"
        )
    namespace, relative_path = (part.strip() for part in parts)
    normalized_namespace = validate_local_namespace(namespace)
    normalized_path = _normalize_cube_relative_path(relative_path)
    return CanonicalCubeId(
        source_kind="local",
        namespace=normalized_namespace,
        path=normalized_path,
    )


def _normalize_cube_relative_path(value: str) -> str:
    """Normalize and validate one source-relative cube file path."""

    if not isinstance(value, str):
        raise CubeIdentityError("Cube id path is invalid")
    cleaned = value.replace("\\", "/").strip().strip("/")
    if not cleaned:
        raise CubeIdentityError("Cube id path is required")
    candidate = PurePosixPath(cleaned)
    if candidate.is_absolute():
        raise CubeIdentityError("Cube id path must be relative")
    if any(part in {"", ".", ".."} for part in candidate.parts):
        raise CubeIdentityError("Cube id path must stay within the managed source")
    normalized = candidate.as_posix()
    if not normalized.lower().endswith(".cube"):
        raise CubeIdentityError("Cube id path must end in '.cube'")
    for segment in candidate.parts:
        _validate_cube_path_segment(segment)
    return normalized


def _validate_cube_path_segment(value: str) -> str:
    """Return an exact cube path segment or reject path-unsafe filename text."""

    if not isinstance(value, str):
        raise CubeIdentityError("Cube id path segment is invalid")
    if not value or value in {".", ".."}:
        raise CubeIdentityError("Cube id path segment is required")
    if "/" in value or "\\" in value:
        raise CubeIdentityError("Cube filename must not contain path separators")
    name_stem = value[:-5] if value.lower().endswith(".cube") else value
    if value.lower().endswith(".cube") and not name_stem:
        raise CubeIdentityError("Cube filename is required")
    if value[-1] in {" ", "."} or name_stem.endswith((" ", ".")):
        raise CubeIdentityError("Cube filename must not end with a space or dot")
    if _WINDOWS_UNSAFE_FILENAME_RE.search(value):
        raise CubeIdentityError("Cube filename contains invalid characters")
    return value
