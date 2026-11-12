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
"""Prove the Python install format satisfies every shared language-neutral vector."""

from __future__ import annotations

import hashlib
import os
from pathlib import Path
from typing import cast

import pytest

from sugarcubes.backend.pack_install.archive import (
    ArchiveLimits,
    extract_registry_archive,
    list_registry_archive,
)
from sugarcubes.backend.pack_install.entry_paths import parse_entry
from sugarcubes.backend.pack_install.errors import PackInstallError
from sugarcubes.backend.pack_install.ownership import (
    OwnedPack,
    changed_tracked_files,
    inspect_pack_folder,
)
from sugarcubes.backend.pack_install.replacement import (
    ReplacementPlan,
    TrackedReplacement,
)
from sugarcubes.backend.pack_install.tracking import (
    parse_tracking,
    read_tracking,
    render_tracking,
    write_tracking,
)
from tests.pack_install.support.install_vectors import (
    JsonObject,
    build_folder,
    folder_files,
    vector_group,
    write_archive,
)


def _ids(group: list[JsonObject]) -> list[str]:
    return [str(vector["name"]) for vector in group]


def _expect(vector: JsonObject) -> JsonObject:
    return cast(JsonObject, vector["expect"])


TRACKING_PARSE = vector_group("trackingParse")
TRACKING_RENDER = vector_group("trackingRender")
ARCHIVES = vector_group("archives")
ARCHIVE_LIMITS = vector_group("archiveLimits")
OWNERSHIP = vector_group("ownership")
LOCAL_EDITS = vector_group("localEdits")
REPLACEMENTS = vector_group("replacements")


@pytest.mark.parametrize("vector", TRACKING_PARSE, ids=_ids(TRACKING_PARSE))
def test_tracking_parse_vectors(vector: JsonObject) -> None:
    """Each record either yields the listed entries or the listed typed error."""

    content = str(vector["text"]).encode("utf-8")
    expected = _expect(vector)
    if "error" in expected:
        with pytest.raises(PackInstallError) as raised:
            parse_tracking(content)
        assert raised.value.code.value == expected["error"]
        return
    assert [entry.text for entry in parse_tracking(content)] == expected["entries"]


@pytest.mark.parametrize("vector", TRACKING_RENDER, ids=_ids(TRACKING_RENDER))
def test_tracking_render_vectors(vector: JsonObject, tmp_path: Path) -> None:
    """The record keeps archive order and uses the platform separator on disk."""

    entries = tuple(parse_entry(text) for text in cast(list[str], vector["entries"]))
    assert render_tracking(entries) == vector["lfText"]
    write_tracking(tmp_path, entries)
    expected = vector["crlfText"] if os.linesep == "\r\n" else vector["lfText"]
    assert (tmp_path / ".tracking").read_bytes() == str(expected).encode("utf-8")
    assert read_tracking(tmp_path) == entries


@pytest.mark.parametrize("vector", ARCHIVES, ids=_ids(ARCHIVES))
def test_archive_vectors(vector: JsonObject, tmp_path: Path) -> None:
    """Archives are listed in order or refused with the vector's typed error."""

    archive = write_archive(
        tmp_path / "pack.zip", cast(list[JsonObject], vector["members"])
    )
    expected = _expect(vector)
    if "error" in expected:
        with pytest.raises(PackInstallError) as raised:
            extract_registry_archive(archive, tmp_path / "out")
        assert raised.value.code.value == expected["error"]
        return
    extracted = extract_registry_archive(archive, tmp_path / "out")
    assert [entry.text for entry in extracted.entries] == expected["entries"]
    for member in cast(list[JsonObject], vector["members"]):
        if member["kind"] == "file":
            text = str(member.get("text", ""))
            assert (
                extracted.file_digests[str(member["name"])]
                == hashlib.sha256(text.encode("utf-8")).hexdigest()
            )


@pytest.mark.parametrize("vector", ARCHIVE_LIMITS, ids=_ids(ARCHIVE_LIMITS))
def test_archive_limit_vectors(vector: JsonObject, tmp_path: Path) -> None:
    """Size and count bounds refuse an archive before it can fill the disk."""

    limits_spec = cast(dict[str, int], vector["limits"])
    limits = ArchiveLimits(
        max_entries=limits_spec["maxEntries"],
        max_total_bytes=limits_spec["maxTotalBytes"],
        max_file_bytes=limits_spec["maxFileBytes"],
    )
    archive = write_archive(
        tmp_path / "pack.zip", cast(list[JsonObject], vector["members"])
    )
    with pytest.raises(PackInstallError) as raised:
        list_registry_archive(archive, limits)
    assert raised.value.code.value == _expect(vector)["error"]


@pytest.mark.parametrize("vector", OWNERSHIP, ids=_ids(OWNERSHIP))
def test_ownership_vectors(vector: JsonObject, tmp_path: Path) -> None:
    """Only a plain registry install is owned; everything else says why not."""

    pack = build_folder(tmp_path, cast(JsonObject, vector["folder"]))
    verdict = inspect_pack_folder(pack)
    expected = _expect(vector)
    if expected["owned"]:
        assert isinstance(verdict, OwnedPack)
        assert str(verdict.identity.version) == expected["version"]
        return
    assert not isinstance(verdict, OwnedPack)
    assert verdict.reason.value == expected["reason"]


@pytest.mark.parametrize("vector", LOCAL_EDITS, ids=_ids(LOCAL_EDITS))
def test_local_edit_vectors(vector: JsonObject) -> None:
    """A tracked file differing from the published release is a local edit."""

    def digest(text: str) -> str:
        return hashlib.sha256(text.encode("utf-8")).hexdigest()

    installed = {
        path: None if text is None else digest(str(text))
        for path, text in cast(dict[str, object], vector["installed"]).items()
    }
    published = {
        path: digest(text)
        for path, text in cast(dict[str, str], vector["published"]).items()
    }
    assert list(changed_tracked_files(installed, published)) == vector["expectChanged"]


@pytest.mark.parametrize("vector", REPLACEMENTS, ids=_ids(REPLACEMENTS))
def test_replacement_vectors(vector: JsonObject, tmp_path: Path) -> None:
    """Replacing tracked files leaves exactly the vector's resulting folder."""

    pack = build_folder(tmp_path, cast(JsonObject, vector["installed"]))
    owned = inspect_pack_folder(pack)
    assert isinstance(owned, OwnedPack)
    archive = write_archive(
        tmp_path / "release.zip", cast(list[JsonObject], vector["release"])
    )
    staged = extract_registry_archive(archive, tmp_path / "staged")
    TrackedReplacement(tmp_path / "transaction").apply(
        ReplacementPlan(
            target=pack,
            source=tmp_path / "staged",
            old_entries=owned.entries,
            new_entries=staged.entries,
            from_version="1.0.0",
            to_version="1.1.0",
        )
    )
    expected = _expect(vector)
    files = folder_files(pack)
    tracking = files.pop(".tracking")
    assert files == expected["files"]
    for absent in cast(list[str], expected["absent"]):
        assert not (pack / absent).exists()
    assert [line for line in tracking.splitlines() if line] == expected[
        "trackingEntries"
    ]
