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
"""Build archives and pack folders from the shared install-format vectors."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import zipfile
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import cast

import pytest

VECTORS_PATH = Path(__file__).with_name("registry_install_vectors.json")
JsonObject = dict[str, object]


def load_vectors() -> JsonObject:
    """Return the shared vector document."""

    return cast(JsonObject, json.loads(VECTORS_PATH.read_text(encoding="utf-8")))


def vector_group(name: str) -> list[JsonObject]:
    """Return one named list of vectors."""

    return cast(list[JsonObject], load_vectors()[name])


def drive(group: str) -> pytest.MarkDecorator:
    """Parametrize a test over every vector of `group`, the only way a driver reads its group.

    A group is a list of named vectors, an object whose `cases` is one, or (for `formats`)
    an object of named entries, each given with its key as `name`. The Substitute host's
    Rust suite reads the same group through its own `drive`, and a repository check requires
    every group's driver in both suites to use it, so every vector runs in both.
    """

    value = load_vectors()[group]
    if isinstance(value, dict) and "cases" not in value:
        vectors = [
            {**cast(JsonObject, entry), "name": key}
            for key, entry in cast(JsonObject, value).items()
        ]
    else:
        vectors = cast(
            list[JsonObject],
            value if isinstance(value, list) else cast(JsonObject, value)["cases"],
        )
    return pytest.mark.parametrize(
        "vector", vectors, ids=[str(vector["name"]) for vector in vectors]
    )


def write_archive(path: Path, members: Sequence[Mapping[str, object]]) -> Path:
    """Write a zip whose members follow the vector description, in order."""

    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as archive:
        for member in members:
            name = str(member["name"])
            kind = member["kind"]
            info = zipfile.ZipInfo(name)
            if "unixMode" in member:
                info.external_attr = int(str(member["unixMode"]), 8) << 16
            elif kind == "directory":
                info.external_attr = (0o40755 << 16) | 0x10
            else:
                info.external_attr = 0o100644 << 16
            archive.writestr(info, str(member.get("text", "")))
    return path


def release_archive(path: Path, files: Mapping[str, str]) -> Path:
    """Write a flat registry-layout archive of plain files."""

    return write_archive(
        path,
        [{"name": name, "kind": "file", "text": text} for name, text in files.items()],
    )


def write_files(root: Path, files: Mapping[str, object]) -> None:
    """Create each file of a folder description under `root`."""

    for relative, text in files.items():
        destination = root.joinpath(*relative.split("/"))
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(str(text).encode("utf-8"))


def make_directory_link(link: Path, target: Path) -> None:
    """Create a directory link a normal Windows user can make: a junction."""

    target.mkdir(parents=True, exist_ok=True)
    if sys.platform == "win32":
        subprocess.run(
            ["cmd", "/c", "mklink", "/J", str(link), str(target)],
            check=True,
            capture_output=True,
        )
    else:
        os.symlink(target, link, target_is_directory=True)


def build_folder(root: Path, folder: Mapping[str, object]) -> Path:
    """Build an installed pack folder from an ownership or replacement vector."""

    pack = root / "custom_nodes" / "SugarCubes"
    if folder.get("link"):
        real = root / "elsewhere" / "SugarCubes"
        real.mkdir(parents=True)
        write_files(real, cast(Mapping[str, object], folder.get("files", {})))
        _write_tracking(real, folder.get("tracking"))
        pack.parent.mkdir(parents=True)
        make_directory_link(pack, real)
        return pack
    pack.mkdir(parents=True)
    write_files(pack, cast(Mapping[str, object], folder.get("files", {})))
    _write_tracking(pack, folder.get("tracking"))
    git = folder.get("git")
    if git == "directory":
        (pack / ".git").mkdir()
    elif git == "file":
        (pack / ".git").write_text("gitdir: elsewhere", encoding="utf-8")
    for linked in cast(list[str], folder.get("linkedDirectories", [])):
        make_directory_link(pack / linked, root / "outside" / linked)
    return pack


def _write_tracking(pack: Path, tracking: object) -> None:
    """Write a vector's `.tracking` text verbatim."""

    if isinstance(tracking, str):
        (pack / ".tracking").write_bytes(tracking.encode("utf-8"))


def folder_files(root: Path) -> dict[str, str]:
    """Return every file under `root` as forward-slash path to UTF-8 text."""

    return {
        path.relative_to(root).as_posix(): path.read_text(encoding="utf-8")
        for path in sorted(root.rglob("*"))
        if path.is_file()
    }


__all__ = [
    "VECTORS_PATH",
    "build_folder",
    "drive",
    "folder_files",
    "load_vectors",
    "make_directory_link",
    "release_archive",
    "vector_group",
    "write_archive",
    "write_files",
]
