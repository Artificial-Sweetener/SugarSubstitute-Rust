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
"""Prove exclusive reference-named destinations without replacing existing model bytes."""

from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
import hashlib
from pathlib import Path
from unittest.mock import patch

import pytest

from sugarcubes.model_library.domain.provider_releases import ModelArtifactOffer
from sugarcubes.model_library.infrastructure.artifact_files import ModelArtifactFile
from sugarcubes.model_library.infrastructure.artifact_destinations import (
    ModelArtifactDestination,
)


def transaction(root: Path, content: bytes = b"fixture") -> ModelArtifactFile:
    """Construct a tiny approved artifact without model loading or provider traffic."""
    return ModelArtifactFile(
        str(root),
        ModelArtifactOffer(
            "30",
            "fixture.safetensors",
            hashlib.sha256(content).hexdigest(),
            len(content),
            "https://civitai.com/api/download/models/20",
            "loras",
        ),
    )


@pytest.mark.parametrize("occupied", [1, 3])
def test_existing_names_use_reference_version_suffixes(
    tmp_path: Path, occupied: int
) -> None:
    """Keep every installed collision intact while reserving the first free version slot."""
    originals = [
        "fixture.safetensors",
        "fixture (v20).safetensors",
        "fixture (v20-1).safetensors",
    ]
    for name in originals[:occupied]:
        (tmp_path / name).write_bytes(b"installed")
    file = transaction(tmp_path)
    file.begin()
    file.append(b"fixture")
    expected = (
        "fixture (v20).safetensors" if occupied == 1 else "fixture (v20-2).safetensors"
    )
    assert file.publish() == expected
    assert (tmp_path / expected).read_bytes() == b"fixture"
    assert all(
        (tmp_path / name).read_bytes() == b"installed" for name in originals[:occupied]
    )
    assert not tuple(tmp_path.glob(".sugarcubes-*"))


def test_concurrent_transactions_reserve_separate_slots(tmp_path: Path) -> None:
    """Hold two destination leases simultaneously and publish both without overwriting."""
    first, second = transaction(tmp_path), transaction(tmp_path, b"fixture2")
    with ThreadPoolExecutor(max_workers=2) as workers:
        pending = [workers.submit(file.begin) for file in (first, second)]
        for operation in pending:
            operation.result()
    first.append(b"fixture")
    second.append(b"fixture2")
    names = [first.publish(), second.publish()]
    assert set(names) == {"fixture.safetensors", "fixture (v20).safetensors"}
    assert (tmp_path / names[0]).read_bytes() == b"fixture"
    assert (tmp_path / names[1]).read_bytes() == b"fixture2"
    assert not tuple(tmp_path.glob(".sugarcubes-*"))


def test_aborted_reservation_releases_the_original_name(tmp_path: Path) -> None:
    """Remove only the transaction's temporary bytes and lease so a retry can reuse the slot."""
    cancelled = transaction(tmp_path)
    cancelled.begin()
    cancelled.append(b"fix")
    cancelled.abort()
    assert not tuple(tmp_path.iterdir())
    retry = transaction(tmp_path)
    retry.begin()
    retry.append(b"fixture")
    assert retry.publish() == "fixture.safetensors"


def test_external_destination_race_preserves_external_bytes(tmp_path: Path) -> None:
    """Reject an unexpected final-file writer and clean only the current reservation."""
    file = transaction(tmp_path)
    file.begin()
    (tmp_path / "fixture.safetensors").write_bytes(b"external")
    file.append(b"fixture")
    with pytest.raises(ValueError, match="preserved"):
        file.publish()
    file.abort()
    assert (tmp_path / "fixture.safetensors").read_bytes() == b"external"
    assert not tuple(tmp_path.glob(".sugarcubes-*"))


def test_replaced_lease_is_preserved_on_failure(tmp_path: Path) -> None:
    """Reject a replaced reservation without deleting its new owner's marker."""
    file = transaction(tmp_path)
    file.begin()
    marker = next(tmp_path.glob("*.reserve"))
    marker.write_bytes(b"other-owner")
    file.append(b"fixture")
    with pytest.raises(ValueError, match="reservation changed"):
        file.publish()
    file.abort()
    assert marker.read_bytes() == b"other-owner"
    assert not (tmp_path / "fixture.safetensors").exists()
    assert not tuple(tmp_path.glob("*.download"))


def test_temporary_cleanup_failure_still_releases_the_lease(tmp_path: Path) -> None:
    """Preserve cleanup failure evidence while releasing the independent destination slot."""
    file = transaction(tmp_path)
    file.begin()
    temporary = next(tmp_path.glob("*.download"))
    unlink = Path.unlink

    def reject_temporary(path: Path, *, missing_ok: bool = False) -> None:
        """Simulate a Windows file lock only at the temporary-file boundary."""
        if path == temporary:
            raise PermissionError("fixture file lock")
        unlink(path, missing_ok=missing_ok)

    with patch.object(Path, "unlink", reject_temporary):
        with pytest.raises(PermissionError, match="fixture file lock"):
            file.abort()
    assert temporary.exists()
    assert not tuple(tmp_path.glob("*.reserve"))
    file.abort()
    assert not tuple(tmp_path.iterdir())


def test_openmodeldb_collision_uses_content_version_in_nested_folder(
    tmp_path: Path,
) -> None:
    """Retain the reference's hash-derived version identity and reviewed relative directory."""
    digest = hashlib.sha256(b"fixture").hexdigest()
    folder = tmp_path / "SDXL"
    folder.mkdir()
    (folder / "renamed.pth").write_bytes(b"installed")
    file = ModelArtifactFile(
        str(tmp_path),
        ModelArtifactOffer(
            "30",
            "fixture.pth",
            digest,
            7,
            "https://huggingface.co/fixture/model/resolve/main/fixture.pth",
            "upscale_models",
            provider="openmodeldb",
        ),
        "SDXL/renamed.pth",
    )
    file.begin()
    file.append(b"fixture")
    name = file.publish()
    assert Path(name).as_posix() == f"SDXL/renamed (v{int(digest[:8], 16)}).pth"
    assert (tmp_path / name).read_bytes() == b"fixture"
    assert (folder / "renamed.pth").read_bytes() == b"installed"
    assert not tuple(folder.glob(".sugarcubes-*"))


def test_reservation_exhaustion_leaves_all_existing_files_intact(
    tmp_path: Path,
) -> None:
    """Fail after the reference's bounded candidate count and clean only the token file."""
    original = transaction(tmp_path)
    for name in ["fixture.safetensors"] + [
        f"fixture (v20{'' if index == 0 else f'-{index}'}).safetensors"
        for index in range(1000)
    ]:
        (tmp_path / name).write_bytes(b"installed")
    with pytest.raises(ValueError, match="side-by-side"):
        original.begin()
    original.abort()
    assert len(tuple(tmp_path.iterdir())) == 1001
    assert all(path.read_bytes() == b"installed" for path in tmp_path.iterdir())


def test_oversized_collision_component_fails_before_reserving(tmp_path: Path) -> None:
    """Keep generated destination names within the existing portable path contract."""
    name = "x" * 188 + ".safetensors"
    (tmp_path / name).write_bytes(b"installed")
    destination = ModelArtifactDestination(
        str(tmp_path),
        ModelArtifactOffer(
            "30",
            name,
            hashlib.sha256(b"fixture").hexdigest(),
            7,
            "https://civitai.com/api/download/models/20",
            "loras",
        ),
        None,
    )
    with pytest.raises(ValueError, match="Invalid model artifact destination"):
        destination.reserve()
    destination.release()
    assert not tuple(tmp_path.glob(".sugarcubes-*"))
    assert (tmp_path / name).read_bytes() == b"installed"


def test_case_aliases_reserve_separate_windows_destination_slots(
    tmp_path: Path,
) -> None:
    """Honor the filesystem's case-insensitive names while both leases are still unpublished."""
    first = transaction(tmp_path)
    second = ModelArtifactFile(
        str(tmp_path),
        ModelArtifactOffer(
            "30",
            "FIXTURE.safetensors",
            hashlib.sha256(b"fixture2").hexdigest(),
            8,
            "https://civitai.com/api/download/models/20",
            "loras",
        ),
    )
    first.begin()
    second.begin()
    try:
        first.append(b"fixture")
        second.append(b"fixture2")
        assert first.publish() == "fixture.safetensors"
        assert second.publish() == "FIXTURE (v20).safetensors"
    finally:
        first.abort()
        second.abort()
    assert (tmp_path / "fixture.safetensors").read_bytes() == b"fixture"
    assert (tmp_path / "FIXTURE (v20).safetensors").read_bytes() == b"fixture2"
