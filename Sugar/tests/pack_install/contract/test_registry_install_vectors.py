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
import importlib
import json
import os
from pathlib import Path
from typing import cast

import pytest

from sugarcubes.backend.pack_install.archive import (
    ArchiveLimits,
    extract_registry_archive,
    list_registry_archive,
)
from sugarcubes.backend.pack_install.digest_manifest import (
    DIGEST_MANIFEST_NAME,
    DIGEST_RECORD_VERSION,
    parse_digest_manifest,
    read_digest_manifest,
    render_digest_manifest,
)
from sugarcubes.backend.pack_install.entry_paths import TRACKING_FILE_NAME, parse_entry
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
    TRACKING_FORMAT_VERSION,
    parse_tracking,
    read_tracking,
    render_tracking,
    write_tracking,
)
from tests.pack_install.support.install_vectors import (
    JsonObject,
    build_folder,
    drive,
    folder_files,
    load_vectors,
    write_archive,
)


def _expect(vector: JsonObject) -> JsonObject:
    return cast(JsonObject, vector["expect"])


@drive("trackingParse")
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


@drive("trackingRender")
def test_tracking_render_vectors(vector: JsonObject, tmp_path: Path) -> None:
    """The record keeps archive order and uses the platform separator on disk."""

    entries = tuple(parse_entry(text) for text in cast(list[str], vector["entries"]))
    assert render_tracking(entries) == vector["lfText"]
    write_tracking(tmp_path, entries)
    expected = vector["crlfText"] if os.linesep == "\r\n" else vector["lfText"]
    assert (tmp_path / ".tracking").read_bytes() == str(expected).encode("utf-8")
    assert read_tracking(tmp_path) == entries


@drive("archives")
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


@drive("archiveLimits")
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


@drive("ownership")
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


@drive("localEdits")
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


@drive("replacements")
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
    recorded = read_digest_manifest(pack, version="1.1.0", entries=staged.entries)
    assert recorded == staged.file_digests
    files.pop(".tracking-digests.json")
    assert files == expected["files"]
    for absent in cast(list[str], expected["absent"]):
        assert not (pack / absent).exists()
    assert [line for line in tracking.splitlines() if line] == expected[
        "trackingEntries"
    ]


DIGEST_MANIFESTS = cast(JsonObject, load_vectors()["digestManifests"])
DIGEST_CASES = cast(list[JsonObject], DIGEST_MANIFESTS["cases"])


@drive("digestManifests")
def test_digest_manifest_vectors(vector: JsonObject) -> None:
    """The digest record is trusted only for the exact install it describes."""

    tracking_text = str(DIGEST_MANIFESTS["trackingText"]).encode("utf-8")
    entries = parse_tracking(tracking_text)
    recorded = parse_digest_manifest(
        json.dumps(vector["manifest"]).encode("utf-8"),
        version=str(DIGEST_MANIFESTS["version"]),
        tracking_sha256=hashlib.sha256(tracking_text).hexdigest(),
        entries=entries,
    )
    expected = _expect(vector)
    if expected["applies"]:
        assert recorded == expected["files"]
    else:
        assert recorded is None


def test_digest_record_renders_exactly_the_shared_bytes() -> None:
    """Every implementation writes the same record bytes for the same install."""

    first = cast(JsonObject, DIGEST_CASES[0]["manifest"])
    rendered = render_digest_manifest(
        version=str(DIGEST_MANIFESTS["version"]),
        tracking_sha256=str(first["tracking"]),
        files=cast(dict[str, str], first["files"]),
    )
    assert rendered == DIGEST_MANIFESTS["rendered"]


@drive("formats")
def test_format_versions_match_the_shared_catalog(vector: JsonObject) -> None:
    """The record names and versions written here are the ones the vectors declare."""

    written = {
        "tracking": (TRACKING_FILE_NAME, TRACKING_FORMAT_VERSION),
        "digestRecord": (DIGEST_MANIFEST_NAME, DIGEST_RECORD_VERSION),
    }
    assert (vector["file"], vector["version"]) == written[str(vector["name"])]


# Every vector group and the Python suite that drives each of its vectors. The decision
# groups run end to end through the updater in
# tests/self_update/integration/test_update_decision_vectors.py.
PYTHON_VECTOR_GROUPS = {
    "formats": "test_format_versions_match_the_shared_catalog",
    "trackingParse": "test_tracking_parse_vectors",
    "trackingRender": "test_tracking_render_vectors",
    "archives": "test_archive_vectors",
    "archiveLimits": "test_archive_limit_vectors",
    "ownership": "test_ownership_vectors",
    "localEdits": "test_local_edit_vectors",
    "replacements": "test_replacement_vectors",
    "digestManifests": "test_digest_manifest_vectors",
    "sourceSelection": "test_source_selection_vectors",
    "updateRefusals": "test_update_refusal_vectors",
    "rollbackTriggers": "test_rollback_trigger_vectors",
}


def test_every_vector_group_is_driven_and_every_name_is_unique() -> None:
    """A group added to the shared file fails here until a Python suite drives it."""

    document = load_vectors()
    groups = set(document) - {"schema", "description"}
    assert groups == set(PYTHON_VECTOR_GROUPS)
    for name in groups:
        value = document[name]
        vectors = (
            value
            if isinstance(value, list)
            else cast(JsonObject, value).get("cases", [])
        )
        names = [
            str(cast(JsonObject, vector)["name"])
            for vector in cast(list[object], vectors)
        ]
        assert len(names) == len(set(names)), name
    decisions = importlib.import_module(
        "tests.self_update.integration.test_update_decision_vectors"
    )
    for test in PYTHON_VECTOR_GROUPS.values():
        assert callable(globals().get(test) or getattr(decisions, test, None)), test
