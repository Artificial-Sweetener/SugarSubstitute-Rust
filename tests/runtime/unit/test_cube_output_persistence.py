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
"""Prove Cube saving retains temporary previews and writes only to the selected Comfy root."""

from __future__ import annotations

from collections.abc import Iterator, Sequence
from pathlib import Path
from types import ModuleType, SimpleNamespace
import sys

import numpy as np
from numpy.typing import NDArray
import pytest

from sugarcubes.runtime.cube_output_images import build_cube_output_preview


class ImageItem:
    """Expose a tiny real array through the Comfy tensor adapter contract."""

    shape: Sequence[int] = (2, 3, 3)

    def cpu(self) -> ImageItem:
        """Keep this deterministic fixture on the CPU."""
        return self

    def numpy(self) -> NDArray[np.float32]:
        """Return three-channel image pixels for actual PNG encoding."""
        return np.ones((2, 3, 3), dtype=np.float32)


class ImageBatch:
    """Model one Comfy IMAGE batch without requiring a GPU runtime."""

    shape: Sequence[int] = (1, 2, 3, 3)

    def __iter__(self) -> Iterator[ImageItem]:
        """Yield this batch's sole item."""
        yield ImageItem()

    def __getitem__(self, index: int) -> ImageItem:
        """Expose the first item used for native save-path geometry."""
        return ImageItem()


@pytest.mark.parametrize("saved", [True, False])
def test_output_policy_writes_to_output_or_temp_and_retains_preview(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, saved: bool
) -> None:
    """Encode an actual PNG while routing through the public host filesystem boundary."""
    output = tmp_path / "output"
    temporary = tmp_path / "temp"
    output.mkdir()
    temporary.mkdir()
    paths = ModuleType("folder_paths")
    setattr(paths, "get_output_directory", lambda: str(output))
    setattr(paths, "get_temp_directory", lambda: str(temporary))

    def save_path(
        prefix: str, root: str, width: int, height: int
    ) -> tuple[str, str, int, str, str]:
        """Return the chosen native output root with a deterministic filename."""
        return root, prefix, 1, "", prefix

    setattr(paths, "get_save_image_path", save_path)
    args = ModuleType("comfy.cli_args")
    setattr(args, "args", SimpleNamespace(disable_metadata=False))
    monkeypatch.setitem(sys.modules, "folder_paths", paths)
    monkeypatch.setitem(sys.modules, "comfy.cli_args", args)

    ui, artifacts = build_cube_output_preview(ImageBatch(), save_outputs=saved)

    assert len(artifacts) == 1
    artifact = artifacts[0]
    assert artifact.type == ("output" if saved else "temp")
    assert ui["images"] == [
        {"filename": artifact.filename, "subfolder": "", "type": artifact.type}
    ]
    destination = output if saved else temporary
    assert (destination / artifact.filename).read_bytes().startswith(b"\x89PNG")
    assert list((temporary if saved else output).iterdir()) == []


def test_opaque_outputs_pass_through_without_files() -> None:
    """Keep non-image Cube values available to execution without inventing file persistence."""
    assert build_cube_output_preview("text", save_outputs=True) == ({}, ())
