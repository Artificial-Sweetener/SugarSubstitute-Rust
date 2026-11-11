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
"""Prove exact output receipts and history-independent re-resolution using real isolated files."""

from dataclasses import replace
import hashlib
from pathlib import Path

from PIL import Image, PngImagePlugin
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


def service(root: Path, state: Path) -> OutputArtifactService:
    """Compose the production filesystem authority without loading a Comfy backend or history."""
    return OutputArtifactService(
        HostArtifactFiles(
            ManagedArtifactRegistry(state), {"output": root, "temp": root}
        )
    )


def save(path: Path, *, metadata: bool = False, color: str = "red") -> None:
    """Write distinguishable complete originals through Pillow's real encoder."""
    info = PngImagePlugin.PngInfo()
    if metadata:
        info.add_text(
            "sugarcubes_output",
            '{"schema_version":1,"run_id":"accepted-prompt","source_node_id":"sink","image_index":1}',
        )
    with Image.new("RGB", (7, 5), color) as image:
        if path.suffix == ".png":
            image.save(path, pnginfo=info)
        else:
            image.save(path)


@pytest.mark.parametrize(
    "extension,metadata,mime",
    [
        ("png", False, "image/png"),
        ("png", True, "image/png"),
        ("jpg", False, "image/jpeg"),
        ("webp", False, "image/webp"),
    ],
)
def test_receipt_confirms_actual_encoded_bytes_with_optional_metadata(
    tmp_path: Path, extension: str, metadata: bool, mime: str
) -> None:
    """Preserve metadata-disabled PNG and companion JPEG identities through digest receipts."""
    path = tmp_path / f"image.{extension}"
    save(path, metadata=metadata)
    file = OutputFileReference(path.name, "", "output")
    owner = service(tmp_path, tmp_path / "artifacts.sqlite")
    receipt = owner.receipt(file, ArtifactBudget())
    assert receipt.digest == hashlib.sha256(path.read_bytes()).hexdigest()
    assert receipt.bytes == path.stat().st_size
    assert (receipt.width, receipt.height, receipt.mime, receipt.metadata_embedded) == (
        7,
        5,
        mime,
        metadata,
    )
    assert (
        owner.resolve(ExpectedArtifact(receipt, "accepted-prompt"), ArtifactBudget())
        == receipt
    )


def test_missing_relocated_file_uses_exact_digest_after_history_is_gone(
    tmp_path: Path,
) -> None:
    """Find a renamed file within its existing namespace without a Comfy history dependency."""
    root = tmp_path / "output"
    root.mkdir()
    original = root / "final.png"
    save(original)
    owner = service(root, tmp_path / "artifacts.sqlite")
    receipt = owner.receipt(
        OutputFileReference(original.name, "", "output"), ArtifactBudget()
    )
    moved = root / "moved"
    moved.mkdir()
    original.rename(moved / "renamed.png")
    save(root / "collision.png", color="blue")
    resolved = owner.resolve(ExpectedArtifact(receipt, None), ArtifactBudget())
    assert resolved.file == OutputFileReference("renamed.png", "moved", "output")
    assert resolved.digest == receipt.digest
    assert original.exists() is False


def test_same_filename_changed_bytes_is_refused_even_when_old_bytes_exist_elsewhere(
    tmp_path: Path,
) -> None:
    """Treat overwriting a known descriptor as changed instead of silently replacing its generation."""
    path = tmp_path / "final.png"
    save(path)
    owner = service(tmp_path, tmp_path / "artifacts.sqlite")
    receipt = owner.receipt(
        OutputFileReference(path.name, "", "output"), ArtifactBudget()
    )
    (tmp_path / "old.png").write_bytes(path.read_bytes())
    save(path, color="blue")
    with pytest.raises(ArtifactUnavailable) as failure:
        owner.resolve(ExpectedArtifact(receipt, None), ArtifactBudget())
    assert failure.value.code == "changed"


def test_identical_candidate_bytes_choose_deterministic_relative_descriptor(
    tmp_path: Path,
) -> None:
    """Permit duplicated exact bytes while preventing basename collisions from claiming identity."""
    path = tmp_path / "original.png"
    save(path)
    owner = service(tmp_path, tmp_path / "artifacts.sqlite")
    receipt = owner.receipt(
        OutputFileReference(path.name, "", "output"), ArtifactBudget()
    )
    (tmp_path / "z.png").write_bytes(path.read_bytes())
    (tmp_path / "a.png").write_bytes(path.read_bytes())
    path.unlink()
    assert (
        owner.resolve(ExpectedArtifact(receipt, None), ArtifactBudget()).file.filename
        == "a.png"
    )


def test_managed_external_root_and_temporary_final_have_confirmed_receipts(
    tmp_path: Path,
) -> None:
    """Reuse the registry's external-root authority without turning an opaque token into a scan root."""
    root = tmp_path / "external"
    root.mkdir()
    path = root / "image.png"
    save(path)
    registry = ManagedArtifactRegistry(tmp_path / "artifacts.sqlite")
    managed = registry.register(
        root=root, png=path, jpeg=None, saved=False, width=7, height=5
    )
    owner = OutputArtifactService(
        HostArtifactFiles(registry, {"output": tmp_path / "other"})
    )
    file = OutputFileReference(managed.filename, managed.subfolder, managed.type)
    receipt = owner.receipt(file, ArtifactBudget())
    assert receipt.file.type == "temp"
    path.rename(root / "moved.png")
    with pytest.raises(ArtifactUnavailable) as failure:
        owner.resolve(ExpectedArtifact(receipt, None), ArtifactBudget())
    assert failure.value.code == "missing"


@pytest.mark.parametrize("kind", ["missing", "corrupt", "unsupported", "truncated"])
def test_individual_unavailable_media_does_not_destroy_files(
    tmp_path: Path, kind: str
) -> None:
    """Return specific failure while keeping originals and unrelated outputs intact."""
    path = tmp_path / ("file.mp4" if kind == "unsupported" else "file.png")
    if kind == "corrupt" or kind == "unsupported":
        path.write_bytes(b"invalid media")
    elif kind == "truncated":
        save(path)
        path.write_bytes(path.read_bytes()[:-12])
    owner = service(tmp_path, tmp_path / "artifacts.sqlite")
    before = path.read_bytes() if path.exists() else None
    with pytest.raises(ArtifactUnavailable) as failure:
        owner.receipt(OutputFileReference(path.name, "", "output"), ArtifactBudget())
    assert failure.value.code == ("missing" if kind == "missing" else "unsupported")
    assert (path.read_bytes() if path.exists() else None) == before


def test_exact_receipt_dimension_mismatch_is_refused(tmp_path: Path) -> None:
    """Validate geometry independently of a matching digest and filename."""
    path = tmp_path / "file.png"
    save(path)
    owner = service(tmp_path, tmp_path / "artifacts.sqlite")
    receipt = owner.receipt(
        OutputFileReference(path.name, "", "output"), ArtifactBudget()
    )
    with pytest.raises(ArtifactUnavailable) as failure:
        owner.resolve(
            ExpectedArtifact(replace(receipt, width=8), None), ArtifactBudget()
        )
    assert failure.value.code == "changed"


def test_managed_jpeg_companion_has_its_own_exact_metadata_free_digest(
    tmp_path: Path,
) -> None:
    """Resolve a registered companion's original bytes without substituting canonical PNG identity."""
    png = tmp_path / "canonical.png"
    jpeg = tmp_path / "canonical.jpg"
    save(png, metadata=True)
    save(jpeg)
    registry = ManagedArtifactRegistry(tmp_path / "state.sqlite")
    managed = registry.register(
        root=tmp_path, png=png, jpeg=jpeg, saved=True, width=7, height=5
    )
    owner = OutputArtifactService(HostArtifactFiles(registry, {}))
    receipt = owner.receipt(
        OutputFileReference(jpeg.name, managed.subfolder, "output"), ArtifactBudget()
    )
    assert receipt.digest == hashlib.sha256(jpeg.read_bytes()).hexdigest()
    assert receipt.mime == "image/jpeg"
    assert receipt.metadata_embedded is False
    assert png.read_bytes() != jpeg.read_bytes()


def test_metadata_narrows_known_prompt_rediscovery_without_inventing_node_or_batch_identity(
    tmp_path: Path,
) -> None:
    """Honor real embedded prompt metadata while preserving independent G0 source and batch owners."""
    path = tmp_path / "original.png"
    save(path, metadata=True)
    owner = service(tmp_path, tmp_path / "state.sqlite")
    receipt = owner.receipt(
        OutputFileReference(path.name, "", "output"), ArtifactBudget()
    )
    assert receipt.embedded_prompt_id == "accepted-prompt"
    path.rename(tmp_path / "renamed.png")
    assert (
        owner.resolve(
            ExpectedArtifact(receipt, "accepted-prompt"), ArtifactBudget()
        ).digest
        == receipt.digest
    )
    with pytest.raises(ArtifactUnavailable) as failure:
        owner.resolve(ExpectedArtifact(receipt, "another-prompt"), ArtifactBudget())
    assert failure.value.code == "missing"
