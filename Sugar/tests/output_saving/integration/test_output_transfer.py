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
"""Prove portable transfer selection uses current preferences and validated registered pairs."""

from dataclasses import replace
from pathlib import Path

import pytest
from PIL import Image

from sugarcubes.output_saving.domain.preferences import JpegPreferences, TransferFormat
from sugarcubes.output_saving.infrastructure.runtime import OutputSavingRuntime
from sugarcubes.output_saving.infrastructure.transfer_references import (
    decode_transfer_reference,
)


def test_transfer_chooses_current_preference_and_falls_back_without_companion(
    tmp_path: Path,
) -> None:
    """Select exact JPEG bytes when available without baking preference into an image or renderer."""
    runtime = OutputSavingRuntime(
        state_root=tmp_path / "state",
        output_root=tmp_path / "output",
        temporary_root=tmp_path / "temp",
    )
    root = tmp_path / "output"
    root.mkdir()
    png, jpeg = root / "result.png", root / "result.jpg"
    Image.new("RGB", (4, 3), "blue").save(png)
    Image.new("RGB", (4, 3), "blue").save(jpeg)
    reference = runtime.artifacts.register(
        root=root, png=png, jpeg=jpeg, saved=True, width=4, height=3
    )
    source = decode_transfer_reference(reference.comfy_image())
    assert runtime.transfers.choose(source).file.filename == png.name
    preferences = replace(
        runtime.settings.load(),
        jpeg=JpegPreferences(enabled=True),
        transfer_format=TransferFormat.COMPANION_JPEG,
    )
    runtime.settings.save(preferences)
    selected = runtime.transfers.choose(source)
    assert selected.file.filename == jpeg.name
    assert selected.mime_type == "image/jpeg"
    assert (
        runtime.artifacts.resolve(
            reference.artifact_id, selected.file.filename, selected.file.type
        )
        == jpeg
    )
    runtime.settings.save(replace(preferences, jpeg=JpegPreferences(enabled=False)))
    assert runtime.transfers.choose(source).file.filename == png.name
    runtime.settings.save(preferences)
    jpeg.unlink()
    assert runtime.transfers.choose(source).file.filename == png.name
    png.unlink()
    with pytest.raises(ValueError, match="unavailable"):
        runtime.transfers.choose(source)


def test_transfer_rejects_tampered_registered_reference_and_preserves_native_files(
    tmp_path: Path,
) -> None:
    """Keep native filenames authoritative while rejecting forged managed file identities and types."""
    runtime = OutputSavingRuntime(
        state_root=tmp_path / "state",
        output_root=tmp_path / "output",
        temporary_root=tmp_path / "temp",
    )
    root = tmp_path / "output"
    root.mkdir()
    png = root / "result.png"
    Image.new("RGB", (2, 2)).save(png)
    reference = runtime.artifacts.register(
        root=root, png=png, jpeg=None, saved=True, width=2, height=2
    )
    for change in (
        {"filename": "other.png"},
        {"type": "temp"},
        {"subfolder": "__sugarcubes_managed__/fake"},
    ):
        with pytest.raises(ValueError, match="unavailable"):
            runtime.transfers.choose(
                decode_transfer_reference({**reference.comfy_image(), **change})
            )
    native = decode_transfer_reference(
        {"filename": "native.jpg", "subfolder": "ordinary/nested", "type": "output"}
    )
    selected = runtime.transfers.choose(native)
    assert selected.file == native
    assert selected.mime_type == "image/jpeg"


@pytest.mark.parametrize(
    "value",
    [
        None,
        {},
        {"filename": "../image.png", "type": "output"},
        {"filename": "image.png", "type": []},
        {"filename": "image.png", "subfolder": "C:/output", "type": "output"},
        {"filename": "image.png", "subfolder": "../output", "type": "output"},
        {"filename": "movie.mp4", "type": "output"},
    ],
)
def test_transfer_decoder_rejects_untrusted_paths_and_formats(value: object) -> None:
    """Narrow untrusted transport values before artifact selection or filesystem access."""
    with pytest.raises(ValueError):
        decode_transfer_reference(value)
