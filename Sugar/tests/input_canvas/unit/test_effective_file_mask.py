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
"""Prove previews use native mask output, preserve its frame and reject unsafe file identities."""

from __future__ import annotations

import sys
from io import BytesIO
from pathlib import Path
from types import ModuleType

import numpy as np
import numpy.typing as npt
import pytest
from PIL import Image

from sugarcubes.input_canvas.infrastructure.input_preview_settings import (
    InputPreviewSettings,
)
from sugarcubes.input_canvas.infrastructure.comfy_mask_preview import (
    native_mask_preview,
)


class NativeTensor:
    """Model the external native tensor boundary with actual numeric coverage samples."""

    def __init__(self, values: npt.NDArray[np.float32]) -> None:
        """Retain native batch geometry rather than borrowing source-file dimensions."""
        self.values = values

    @property
    def shape(self) -> tuple[int, ...]:
        """Expose the shape that the installed decoder produced."""
        return self.values.shape

    def detach(self) -> NativeTensor:
        """Model a detached native read."""
        return self

    def cpu(self) -> NativeTensor:
        """Model access to decoded CPU image samples."""
        return self

    def numpy(self) -> npt.NDArray[np.float32]:
        """Supply real array data to the production image encoder."""
        return self.values

    def float(self) -> NativeTensor:
        """Model native intermediate-precision conversion."""
        return self


def native_modules(
    monkeypatch: pytest.MonkeyPatch,
    root: Path,
    *,
    mixed: bool = False,
    ambiguous: bool = False,
) -> list[tuple[str, str]]:
    """Replace only dynamic Comfy imports while retaining validation and PNG conversion."""
    calls: list[tuple[str, str]] = []

    class Loader:
        """Supply distinguishable native alpha and red-channel results."""

        RETURN_TYPES = (
            ("IMAGE", "MASK") if mixed else ("MASK", "MASK") if ambiguous else ("MASK",)
        )
        FUNCTION = "load_image_mask"

        @classmethod
        def INPUT_TYPES(cls) -> dict[str, object]:
            """Expose native file upload and channel choices."""
            return {
                "required": {
                    "image": (["source.png"], {"image_upload": True}),
                    "channel": (["alpha", "red", "green", "blue"],),
                }
            }

        def load_image_mask(self, image: str, channel: str) -> tuple[NativeTensor, ...]:
            """Return native empty-alpha geometry or interpreted soft coverage."""
            calls.append((image, channel))
            values = (
                np.zeros((1, 64, 64), dtype=np.float32)
                if channel == "alpha"
                else np.asarray([[[0, 0.5, 1], [1, 0.25, 0]]], dtype=np.float32)
            )
            return (
                (
                    NativeTensor(np.zeros((1, 1, 1), dtype=np.float32)),
                    NativeTensor(values),
                )
                if mixed
                else (NativeTensor(values),)
            )

    nodes = ModuleType("nodes")
    setattr(
        nodes, "NODE_CLASS_MAPPINGS", {"LoadImageMask": Loader, "LoadImage": Loader}
    )
    tensors = ModuleType("torch")
    setattr(tensors, "Tensor", NativeTensor)
    folders = ModuleType("folder_paths")

    def annotated(name: str) -> tuple[str, None]:
        """Model the host's annotated media reference parser."""
        return name, None

    def directory() -> str:
        """Resolve the isolated test media directory."""
        return str(root)

    setattr(folders, "annotated_filepath", annotated)
    setattr(folders, "get_input_directory", directory)
    for name, module in [
        ("nodes", nodes),
        ("torch", tensors),
        ("folder_paths", folders),
    ]:
        monkeypatch.setitem(sys.modules, name, module)
    return calls


def test_native_mask_output_controls_channel_and_preview_geometry(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Use actual native output instead of showing or interpreting the original RGB photo."""
    Image.new("RGB", (120, 200), "red").save(tmp_path / "source.png")
    calls = native_modules(monkeypatch, tmp_path)
    alpha = Image.open(
        BytesIO(
            native_mask_preview(
                "source.png",
                InputPreviewSettings(
                    "mask", "LoadImageMask", "image", {"channel": "alpha"}
                ),
                288,
            )
        )
    )
    assert alpha.size == (64, 64)
    assert set(alpha.tobytes()) == {0}
    red = Image.open(
        BytesIO(
            native_mask_preview(
                "source.png",
                InputPreviewSettings(
                    "mask", "LoadImageMask", "image", {"channel": "red"}
                ),
                288,
            )
        )
    )
    assert red.size == (3, 2)
    assert list(red.tobytes()) == [0, 128, 255, 255, 64, 0]
    assert calls == [("source.png", "alpha"), ("source.png", "red")]


def test_execution_mask_preserves_native_resolution_and_soft_coverage(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Freeze exact native samples rather than an upscaled preview for returned inpaints."""
    Image.new("RGB", (120, 200), "red").save(tmp_path / "source.png")
    calls = native_modules(monkeypatch, tmp_path)
    settings = InputPreviewSettings(
        "mask", "LoadImageMask", "image", {"channel": "red"}
    )
    full = Image.open(BytesIO(native_mask_preview("source.png", settings, 0)))
    preview = Image.open(BytesIO(native_mask_preview("source.png", settings, 1)))
    assert full.size == (3, 2)
    assert list(full.tobytes()) == [0, 128, 255, 255, 64, 0]
    assert preview.size == (1, 1)
    assert len(calls) == 2


def test_mixed_image_loader_decodes_its_actual_mask_output(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Select the MASK socket rather than mistaking the first IMAGE result for coverage."""
    Image.new("RGB", (120, 200), "red").save(tmp_path / "source.png")
    native_modules(monkeypatch, tmp_path, mixed=True)
    settings = InputPreviewSettings("mask", "LoadImage", "image", {"channel": "red"})
    mask = Image.open(BytesIO(native_mask_preview("source.png", settings, 0)))
    assert mask.size == (3, 2)
    assert list(mask.tobytes()) == [0, 128, 255, 255, 64, 0]


def test_multiple_native_mask_outputs_are_not_guessed(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Refuse an ambiguous source before interpreting a different mask than the queued graph."""
    Image.new("RGB", (120, 200), "red").save(tmp_path / "source.png")
    calls = native_modules(monkeypatch, tmp_path, ambiguous=True)
    settings = InputPreviewSettings(
        "mask", "LoadImageMask", "image", {"channel": "red"}
    )
    with pytest.raises(ValueError, match="unambiguous"):
        native_mask_preview("source.png", settings, 0)
    assert calls == []


def test_file_preview_rejects_traversal_and_invalid_settings_before_native_loading(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Keep external filenames inside the media directory and loader settings within supported bounds."""
    root = tmp_path / "input"
    root.mkdir()
    (tmp_path / "private.png").write_bytes(b"private")
    calls = native_modules(monkeypatch, root)
    with pytest.raises(ValueError, match="media directory"):
        native_mask_preview(
            "../private.png",
            InputPreviewSettings("mask", "LoadImageMask", "image", {"channel": "red"}),
            288,
        )
    with pytest.raises(FileNotFoundError):
        native_mask_preview(
            "missing.png",
            InputPreviewSettings("mask", "LoadImageMask", "image", {"channel": "red"}),
            288,
        )
    (root / "source.png").write_bytes(b"source")
    with pytest.raises(ValueError, match="choice"):
        native_mask_preview(
            "source.png",
            InputPreviewSettings(
                "mask", "LoadImageMask", "image", {"channel": "luminance"}
            ),
            288,
        )
    with pytest.raises(ValueError, match="size"):
        native_mask_preview(
            "missing.png",
            InputPreviewSettings(
                "mask", "LoadImageMask", "image", {"channel": "alpha"}
            ),
            4096,
        )
    assert calls == []


def test_custom_loader_preview_uses_native_settings_and_mask_output_slot(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    """Honor custom threshold/invert processing instead of assuming every loader extracts a channel."""
    Image.new("RGB", (3, 2), "red").save(tmp_path / "source.png")
    native_modules(monkeypatch, tmp_path)
    nodes = sys.modules["nodes"]

    class Loader:
        """Model a registered custom file loader with a secondary non-image output."""

        RETURN_TYPES = ("STRING", "MASK")
        FUNCTION = "load"

        @classmethod
        def INPUT_TYPES(cls) -> dict[str, object]:
            """Declare actual native primitive processing settings."""
            return {
                "required": {
                    "file": (["source.png"], {"image_upload": True}),
                    "threshold": ("FLOAT", {"min": 0.0, "max": 1.0}),
                    "invert": ("BOOLEAN",),
                }
            }

        def load(self, file: str, threshold: float, invert: bool) -> dict[str, object]:
            """Produce native processed coverage through a real ndarray boundary."""
            assert file == "source.png"
            values = np.asarray([[[0.2, 0.7]]], dtype=np.float32)
            values = (values >= threshold).astype(np.float32)
            if invert:
                values = 1 - values
            return {"result": ("metadata", NativeTensor(values))}

    setattr(nodes, "NODE_CLASS_MAPPINGS", {"CustomMask": Loader})
    settings = InputPreviewSettings(
        "mask", "CustomMask", "file", {"threshold": 0.5, "invert": True}
    )
    image = Image.open(BytesIO(native_mask_preview("source.png", settings, 288)))
    assert image.size == (2, 1)
    assert list(image.tobytes()) == [255, 0]
    for invalid in (
        InputPreviewSettings(
            "mask", "CustomMask", "file", {"threshold": 2, "invert": True}
        ),
        InputPreviewSettings("mask", "CustomMask", "file", {"threshold": 0.5}),
        InputPreviewSettings(
            "mask", "CustomMask", "wrong", {"threshold": 0.5, "invert": False}
        ),
        InputPreviewSettings("mask", "Unknown", "file", {}),
    ):
        with pytest.raises(ValueError):
            native_mask_preview("source.png", invalid, 288)
