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
"""Prove work bounds and linked-path refusal at the real filesystem authority."""

from pathlib import Path
import subprocess
import sys
from threading import Event

from PIL import Image
import pytest

from sugarcubes.output_saving.application.artifact_service import OutputArtifactService
from sugarcubes.output_saving.domain.artifact import (
    ArtifactUnavailable,
    ExpectedArtifact,
)
from sugarcubes.output_saving.domain.transfer import OutputFileReference
from sugarcubes.output_saving.infrastructure.artifact_budget import ArtifactBudget
from sugarcubes.output_saving.infrastructure.artifact_files import HostArtifactFiles
from sugarcubes.output_saving.infrastructure.artifact_registry import (
    ManagedArtifactRegistry,
)
from sugarcubes.output_saving.infrastructure.contained_files import open_contained


def test_cancel_and_deadline_are_observed_before_filesystem_read(
    tmp_path: Path,
) -> None:
    """Refuse work without requiring sleeps, a running backend or any active native handles."""
    owner = OutputArtifactService(
        HostArtifactFiles(
            ManagedArtifactRegistry(tmp_path / "state.sqlite"), {"output": tmp_path}
        )
    )
    cancellation = Event()
    cancellation.set()
    for budget, code in [
        (ArtifactBudget(cancelled=cancellation), "cancelled"),
        (ArtifactBudget(seconds=0), "budget"),
    ]:
        with pytest.raises(ArtifactUnavailable) as failure:
            owner.receipt(OutputFileReference("missing.png", "", "output"), budget)
        assert failure.value.code == code


def test_exact_candidate_and_metadata_limits_fail_closed(tmp_path: Path) -> None:
    """Charge every candidate and bounded header read while leaving user originals intact."""
    path = tmp_path / "file.png"
    with Image.new("RGB", (4, 3)) as image:
        image.save(path)
    files = HostArtifactFiles(
        ManagedArtifactRegistry(tmp_path / "state.sqlite"), {"output": tmp_path}
    )
    receipt = files.receipt(
        OutputFileReference(path.name, "", "output"), ArtifactBudget()
    )
    with pytest.raises(ArtifactUnavailable) as failure:
        files.receipt(receipt.file, ArtifactBudget(maximum_metadata=1))
    assert failure.value.code == "budget"
    path.rename(tmp_path / "renamed.png")
    with pytest.raises(ArtifactUnavailable) as failure:
        files.rediscover(
            ExpectedArtifact(receipt, None), ArtifactBudget(maximum_candidates=0)
        )
    assert failure.value.code == "budget"
    exact = ArtifactBudget(maximum_candidates=1, maximum_metadata=1)
    exact.candidate()
    exact.metadata(1)
    with pytest.raises(ArtifactUnavailable):
        exact.candidate()
    with pytest.raises(ArtifactUnavailable):
        ArtifactBudget(maximum_metadata=0).metadata(1)


def test_relative_root_and_traversal_never_grant_file_authority(tmp_path: Path) -> None:
    """Reject invalid configured roots independently of descriptor validation at HTTP."""
    registry = ManagedArtifactRegistry(tmp_path / "state.sqlite")
    with pytest.raises(ValueError):
        HostArtifactFiles(registry, {"output": Path("relative")})
    files = HostArtifactFiles(registry, {"output": tmp_path})
    with pytest.raises(ArtifactUnavailable) as failure:
        files.receipt(
            OutputFileReference("file.png", "../outside", "output"), ArtifactBudget()
        )
    assert failure.value.code == "invalid_reference"


def test_in_root_junction_or_symlink_is_refused_before_read(tmp_path: Path) -> None:
    """Reject linked ancestry even when its target happens to remain inside the approved root."""
    target = tmp_path / "real"
    target.mkdir()
    path = target / "file.png"
    with Image.new("RGB", (4, 3)) as image:
        image.save(path)
    link = tmp_path / "linked"
    if sys.platform == "win32":
        subprocess.run(
            ["cmd.exe", "/c", "mklink", "/J", str(link), str(target)],
            check=True,
            capture_output=True,
            creationflags=subprocess.CREATE_NO_WINDOW,
        )
    else:
        link.symlink_to(target, target_is_directory=True)
    files = HostArtifactFiles(
        ManagedArtifactRegistry(tmp_path / "state.sqlite"), {"output": tmp_path}
    )
    try:
        with pytest.raises(ArtifactUnavailable) as failure:
            files.receipt(
                OutputFileReference("file.png", "linked", "output"), ArtifactBudget()
            )
        assert failure.value.code == "invalid_reference"
        admitted = HostArtifactFiles(
            ManagedArtifactRegistry(tmp_path / "other.sqlite"), {"output": link}
        ).receipt(OutputFileReference("file.png", "", "output"), ArtifactBudget())
        assert (admitted.width, admitted.height) == (4, 3)
    finally:
        if sys.platform == "win32":
            link.rmdir()
        else:
            link.unlink()
    assert path.is_file()


def test_opened_regular_handle_detects_path_replacement(tmp_path: Path) -> None:
    """Keep the actual original pinned and reject a replacement descriptor after reading."""
    path = tmp_path / "file.png"
    path.write_bytes(b"original")
    with pytest.raises((ValueError, PermissionError)):
        with open_contained(tmp_path, path) as stream:
            assert stream.read() == b"original"
            path.rename(tmp_path / "moved.png")
            path.write_bytes(b"replaced")
    assert (
        (tmp_path / "moved.png").read_bytes() == b"original"
        if (tmp_path / "moved.png").exists()
        else path.read_bytes() == b"original"
    )


def test_active_writer_cannot_be_confirmed_as_a_stable_final(tmp_path: Path) -> None:
    """Refuse write custody on Windows and still verify normal original reads on other platforms."""
    path = tmp_path / "final.png"
    with Image.new("RGB", (4, 3), "red") as image:
        image.save(path)
    files = HostArtifactFiles(
        ManagedArtifactRegistry(tmp_path / "state.sqlite"), {"output": tmp_path}
    )
    with path.open("r+b"):
        if sys.platform == "win32":
            with pytest.raises(ArtifactUnavailable) as failure:
                files.receipt(
                    OutputFileReference(path.name, "", "output"), ArtifactBudget()
                )
            assert failure.value.code == "inaccessible"
        else:
            assert (
                files.receipt(
                    OutputFileReference(path.name, "", "output"), ArtifactBudget()
                ).width
                == 4
            )
    assert (
        files.receipt(
            OutputFileReference(path.name, "", "output"), ArtifactBudget()
        ).width
        == 4
    )
