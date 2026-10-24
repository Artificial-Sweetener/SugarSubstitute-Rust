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
"""Prove file image previews preserve native colors, alpha and geometry without running image-processing nodes."""

from __future__ import annotations
import sys
from io import BytesIO
from pathlib import Path
from types import ModuleType
import numpy as np
import numpy.typing as npt
import pytest
from PIL import Image
from sugarcubes.input_canvas.infrastructure.comfy_image_preview import (
    native_image_preview,
)
from sugarcubes.input_canvas.infrastructure.input_preview_settings import (
    InputPreviewSettings,
)


class Tensor:
    """Model only the native tensor boundary using real floating-point image samples."""

    def __init__(self, values: npt.NDArray[np.float32]) -> None:
        """Retain decoded native source data."""
        self.values = values

    @property
    def shape(self) -> tuple[int, ...]:
        """Expose native image or alpha geometry."""
        return self.values.shape

    def detach(self) -> Tensor:
        """Model read-only native sample access."""
        return self

    def cpu(self) -> Tensor:
        """Model transfer to the codec worker."""
        return self

    def float(self) -> Tensor:
        """Model native precision normalization."""
        return self

    def numpy(self) -> npt.NDArray[np.float32]:
        """Expose actual source samples to the production PNG encoder."""
        return self.values


def test_file_images_use_native_codec_and_retain_source_frame_and_alpha(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Preview source file content while leaving graph-driven image resizing/compositing to its own node."""
    Image.new("RGB", (2, 1)).save(tmp_path / "source.png")
    calls: list[str] = []

    class Codec:
        """Supply distinguishable native image colors and inverse-alpha coverage."""

        @classmethod
        def INPUT_TYPES(cls) -> dict[str, object]:
            """Declare the ordinary native file upload field."""
            return {"required": {"image": (["source.png"], {"image_upload": True})}}

        def load_image(self, image: str) -> tuple[Tensor, Tensor]:
            """Return native decoded values independently from the original fixture's pixels."""
            calls.append(image)
            return Tensor(
                np.asarray([[[[1, 0, 0], [0, 1, 0]]]], dtype=np.float32)
            ), Tensor(np.asarray([[[0, 0.5]]], dtype=np.float32))

    class ProcessingNode:
        """Represent an image upload field whose processing requires another graph input."""

        @classmethod
        def INPUT_TYPES(cls) -> dict[str, object]:
            """Expose a source alongside graph-owned processing inputs."""
            return {
                "required": {
                    "reference": (["source.png"], {"image_upload": True}),
                    "base_image": ("IMAGE",),
                }
            }

        def execute(self) -> None:
            """Reject attempts to run image processing merely to display its uploaded source."""
            raise AssertionError("Preview must not run the processing graph.")

    nodes = ModuleType("nodes")
    setattr(
        nodes, "NODE_CLASS_MAPPINGS", {"LoadImage": Codec, "Composite": ProcessingNode}
    )
    tensors = ModuleType("torch")
    setattr(tensors, "Tensor", Tensor)
    folders = ModuleType("folder_paths")

    def annotated(file: str) -> tuple[str, None]:
        """Resolve one typed native media reference."""
        return file, None

    def directory() -> str:
        """Retain the isolated native media root."""
        return str(tmp_path)

    setattr(folders, "annotated_filepath", annotated)
    setattr(folders, "get_input_directory", directory)
    for name, module in [
        ("nodes", nodes),
        ("torch", tensors),
        ("folder_paths", folders),
    ]:
        monkeypatch.setitem(sys.modules, name, module)
    image = Image.open(
        BytesIO(
            native_image_preview(
                "source.png",
                InputPreviewSettings("image", "Composite", "reference", {}),
                288,
            )
        )
    )
    assert image.mode == "RGBA" and image.size == (2, 1)
    assert list(image.tobytes()) == [255, 0, 0, 255, 0, 255, 0, 128]
    assert calls == ["source.png"]
    with pytest.raises(ValueError, match="upload"):
        native_image_preview(
            "source.png",
            InputPreviewSettings("image", "Composite", "base_image", {}),
            288,
        )
    with pytest.raises(ValueError, match="media directory"):
        native_image_preview(
            "../outside.png",
            InputPreviewSettings("image", "LoadImage", "image", {}),
            288,
        )
