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
"""Prove animated and video finals retain exact identity through safe bounded output acquisition."""

from dataclasses import replace
import hashlib
from io import BytesIO
from pathlib import Path
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
from tests.model_library.support.preview_media import animated_preview


def encoded_media(kind: str, *, reversed_frames: bool = False) -> bytes:
    """Encode real tiny animation and video containers through installed production codecs."""
    if kind in {"mp4", "webm", "gif"}:
        return animated_preview(
            kind,
            first_color=b"\x00\xff\x00\xff" if reversed_frames else b"\xff\x00\x00\xff",
        )
    target = BytesIO()
    with (
        Image.new("RGB", (64, 96), "green" if reversed_frames else "red") as first,
        Image.new("RGB", (64, 96), "blue") as second,
    ):
        first.save(
            target,
            format="WEBP" if kind == "webp" else "PNG",
            save_all=True,
            append_images=[second],
            duration=100,
            loop=0,
        )
    return target.getvalue()


@pytest.mark.parametrize(
    "kind,mime,media,animated",
    [
        ("gif", "image/gif", "image", True),
        ("webp", "image/webp", "image", True),
        ("png", "image/png", "image", True),
        ("apng", "image/png", "image", True),
        ("mp4", "video/mp4", "video", False),
        ("webm", "video/webm", "video", False),
    ],
)
def test_media_receipt_move_and_changed_identity(
    kind: str, mime: str, media: str, animated: bool, tmp_path: Path
) -> None:
    """Restore exact files after relocation without confusing animation routing with G0 media kind."""
    path = tmp_path / f"original.{kind}"
    encoded = encoded_media(kind)
    path.write_bytes(encoded)
    owner = OutputArtifactService(
        HostArtifactFiles(
            ManagedArtifactRegistry(tmp_path / "state.sqlite"), {"output": tmp_path}
        )
    )
    receipt = owner.receipt(
        OutputFileReference(path.name, "", "output"), ArtifactBudget()
    )
    assert receipt.digest == hashlib.sha256(encoded).hexdigest()
    assert receipt.bytes == len(encoded)
    assert (
        receipt.width,
        receipt.height,
        receipt.mime,
        receipt.media,
        receipt.animated,
    ) == (64, 96, mime, media, animated)
    path.write_bytes(encoded_media(kind, reversed_frames=True))
    with pytest.raises(ArtifactUnavailable) as changed:
        owner.resolve(ExpectedArtifact(receipt, None), ArtifactBudget())
    assert changed.value.code == "changed"
    path.write_bytes(encoded)
    moved = tmp_path / f"moved.{kind}"
    path.rename(moved)
    resolved = owner.resolve(ExpectedArtifact(receipt, None), ArtifactBudget())
    assert resolved.file.filename == moved.name
    assert (resolved.digest, resolved.animated) == (receipt.digest, animated)
    with pytest.raises(ArtifactUnavailable) as failure:
        owner.resolve(
            ExpectedArtifact(replace(resolved, digest="0" * 64), None), ArtifactBudget()
        )
    assert failure.value.code == "changed"
    moved.unlink()
    with pytest.raises(ArtifactUnavailable) as failure:
        owner.resolve(ExpectedArtifact(receipt, None), ArtifactBudget())
    assert failure.value.code == "missing"
    cancelled = Event()
    cancelled.set()
    with pytest.raises(ArtifactUnavailable) as failure:
        owner.receipt(receipt.file, ArtifactBudget(cancelled=cancelled))
    assert failure.value.code == "cancelled"


@pytest.mark.parametrize("kind,animated", [("gif", True), ("webp", False)])
def test_single_frame_routing_matches_existing_presenter_semantics(
    kind: str, animated: bool, tmp_path: Path
) -> None:
    """Keep GIF on its established animated presenter while static WebP stays a still image."""
    path = tmp_path / f"single.{kind}"
    with Image.new("RGB", (4, 3), "red") as image:
        image.save(path)
    files = HostArtifactFiles(
        ManagedArtifactRegistry(tmp_path / "state.sqlite"), {"output": tmp_path}
    )
    assert (
        files.receipt(
            OutputFileReference(path.name, "", "output"), ArtifactBudget()
        ).animated
        is animated
    )


@pytest.mark.parametrize("kind", ["mp4", "webm"])
def test_video_probe_metadata_admission_remains_finite(
    kind: str, tmp_path: Path
) -> None:
    """Refuse a video header outside the operation budget without loading full media into application state."""
    path = tmp_path / f"clip.{kind}"
    path.write_bytes(encoded_media(kind))
    files = HostArtifactFiles(
        ManagedArtifactRegistry(tmp_path / "state.sqlite"), {"output": tmp_path}
    )
    with pytest.raises(ArtifactUnavailable) as failure:
        files.receipt(
            OutputFileReference(path.name, "", "output"),
            ArtifactBudget(maximum_metadata=1),
        )
    assert failure.value.code == "budget"
