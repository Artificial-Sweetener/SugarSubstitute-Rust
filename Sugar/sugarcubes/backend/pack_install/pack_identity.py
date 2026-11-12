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
"""Read the identity a pack folder declares in its `pyproject.toml`.

ComfyUI Manager recognises a registry install by `pyproject.toml` beside
`.tracking`, and reads the pack id and version from `[project]`.
"""

from __future__ import annotations

import re
import tomllib
from dataclasses import dataclass
from pathlib import Path

from .errors import PackInstallError, PackInstallErrorCode

_RELEASE_VERSION = re.compile(
    r"^(0|[1-9][0-9]{0,8})\.(0|[1-9][0-9]{0,8})\.(0|[1-9][0-9]{0,8})$"
)
_MAX_PYPROJECT_BYTES = 256 * 1024


@dataclass(frozen=True, slots=True, order=True)
class ReleaseVersion:
    """Order plain `major.minor.patch` release versions numerically."""

    major: int
    minor: int
    patch: int

    def __str__(self) -> str:
        """Return the canonical release token."""

        return f"{self.major}.{self.minor}.{self.patch}"


def parse_release_version(value: object) -> ReleaseVersion:
    """Return one strict release version or raise a typed identity failure."""

    if not isinstance(value, str):
        raise _identity("The version is not text.")
    match = _RELEASE_VERSION.fullmatch(value)
    if match is None:
        raise _identity("The version is not a plain release version.")
    return ReleaseVersion(int(match[1]), int(match[2]), int(match[3]))


@dataclass(frozen=True, slots=True)
class PackIdentity:
    """Carry the pack id and release version a folder declares."""

    name: str
    version: ReleaseVersion


def read_pack_identity(folder: Path) -> PackIdentity:
    """Return the `[project]` name and version of one pack folder."""

    pyproject = folder / "pyproject.toml"
    if not (folder / "__init__.py").is_file() or not pyproject.is_file():
        raise _identity("The folder is not a ComfyUI node pack.")
    try:
        with pyproject.open("rb") as handle:
            content = handle.read(_MAX_PYPROJECT_BYTES + 1)
        if len(content) > _MAX_PYPROJECT_BYTES:
            raise _identity("The pack metadata exceeds the size limit.")
        payload = tomllib.loads(content.decode("utf-8"))
    except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError) as exc:
        raise _identity("The pack metadata is unreadable.") from exc
    project = payload.get("project")
    if not isinstance(project, dict):
        raise _identity("The pack metadata has no project table.")
    name = project.get("name")
    if not isinstance(name, str) or not name.strip():
        raise _identity("The pack metadata has no name.")
    return PackIdentity(
        name=name.strip(), version=parse_release_version(project.get("version"))
    )


def require_pack_identity(
    folder: Path, *, pack_id: str, version: ReleaseVersion | None
) -> PackIdentity:
    """Return the folder's identity after proving it is the expected pack."""

    identity = read_pack_identity(folder)
    if identity.name.casefold() != pack_id.casefold():
        raise _identity(f"The archive is not the {pack_id} pack.")
    if version is not None and identity.version != version:
        raise _identity(f"The archive is not {pack_id} {version}.")
    return identity


def _identity(detail: str) -> PackInstallError:
    """Build one pack identity rejection."""

    return PackInstallError(PackInstallErrorCode.PACK_IDENTITY, detail)


__all__ = [
    "PackIdentity",
    "ReleaseVersion",
    "parse_release_version",
    "read_pack_identity",
    "require_pack_identity",
]
