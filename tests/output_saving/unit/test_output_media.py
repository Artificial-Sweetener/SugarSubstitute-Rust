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
"""Prove full-resolution transient renditions, shared preference ordering and contained file reveal."""

from dataclasses import dataclass, replace
from io import BytesIO
from pathlib import Path

import pytest
from PIL import Image

from sugarcubes.output_saving.application.media_service import OutputMediaService
from sugarcubes.output_saving.domain.media import ImageTransferFormat
from sugarcubes.output_saving.domain.preferences import (
    OutputSavingPreferences,
    JpegPreferences,
    TransferFormat,
)
from sugarcubes.output_saving.domain.transfer import OutputFileReference
from sugarcubes.output_saving.infrastructure.artifact_registry import (
    ManagedArtifactRegistry,
)
from sugarcubes.output_saving.infrastructure.media_files import HostOutputImageFiles
from sugarcubes.output_saving.infrastructure.transfer_references import (
    decode_transfer_reference,
)


@dataclass
class Revealer:
    """Record the operating-system boundary without replacing image resolution or encoding."""

    paths: list[Path]

    @property
    def available(self) -> bool:
        """Supply an installed desktop for platform-neutral policy assertions."""
        return True

    def reveal(self, path: Path) -> None:
        """Record exactly the validated file forwarded to the desktop."""
        self.paths.append(path)


def fixture(
    root: Path,
) -> tuple[
    OutputMediaService,
    HostOutputImageFiles,
    list[OutputSavingPreferences],
    list[Path],
    Path,
]:
    """Use real native and managed image files while substituting only the desktop launch."""
    output = root / "output"
    output.mkdir()
    png = output / "sample.png"
    with Image.new("RGB", (64, 32)) as image:
        image.putdata(
            [
                (x * 17 % 256, y * 39 % 256, (x + y) * 11 % 256)
                for y in range(32)
                for x in range(64)
            ]
        )
        image.save(png)
    paths: list[Path] = []
    files = HostOutputImageFiles(
        ManagedArtifactRegistry(root / "state.sqlite"),
        {"output": output, "temp": root / "temp"},
        Revealer(paths),
    )
    settings = [OutputSavingPreferences()]
    return OutputMediaService(lambda: settings[0], files), files, settings, paths, png


def test_one_time_jpeg_uses_retained_quality_without_creating_companions(
    tmp_path: Path,
) -> None:
    """Use default quality 100 and an explicitly retained disabled-companion quality on demand."""
    service, _, settings, _, png = fixture(tmp_path)
    reference = OutputFileReference(png.name, "", "output")
    exact = service.prepare(reference, ImageTransferFormat.PNG)
    assert exact.data == png.read_bytes()
    high = service.prepare(reference, ImageTransferFormat.JPEG)
    settings[0] = replace(settings[0], jpeg=JpegPreferences(enabled=False, quality=35))
    low = service.prepare(reference, ImageTransferFormat.JPEG)
    assert low.mime_type == "image/jpeg"
    assert low.filename == "sample.jpg"
    assert len(low.data) < len(high.data)
    with Image.open(BytesIO(low.data)) as image:
        assert image.size == (64, 32)
    assert not list(png.parent.glob("*.jpg"))
    assert not settings[0].jpeg.enabled


def test_preferred_copy_changes_with_drag_settings_and_creates_missing_jpeg_on_demand(
    tmp_path: Path,
) -> None:
    """Keep primary Copy shared with enabled drag preference and explicit PNG independent."""
    service, _, settings, _, png = fixture(tmp_path)
    reference = OutputFileReference(png.name, "", "output")
    assert service.profile()[0] is ImageTransferFormat.PNG
    settings[0] = replace(
        settings[0],
        jpeg=JpegPreferences(enabled=True, quality=77),
        transfer_format=TransferFormat.COMPANION_JPEG,
    )
    assert service.profile()[0] is ImageTransferFormat.JPEG
    assert (
        service.prepare(reference, ImageTransferFormat.PREFERRED).mime_type
        == "image/jpeg"
    )
    assert service.prepare(reference, ImageTransferFormat.PNG).data == png.read_bytes()
    settings[0] = replace(settings[0], jpeg=JpegPreferences(enabled=False, quality=77))
    assert (
        service.prepare(reference, ImageTransferFormat.PREFERRED).mime_type
        == "image/png"
    )


def test_native_temp_reveal_and_path_escape_rejection(tmp_path: Path) -> None:
    """Reveal native preview files and refuse missing or escaped image locations."""
    service, files, _, revealed, png = fixture(tmp_path)
    service.reveal(OutputFileReference(png.name, "", "output"))
    assert revealed == [png.resolve()]
    temporary = tmp_path / "temp"
    temporary.mkdir()
    preview = temporary / "preview.png"
    preview.write_bytes(png.read_bytes())
    service.reveal(OutputFileReference(preview.name, "", "temp"))
    assert revealed[-1] == preview.resolve()
    for reference in [
        OutputFileReference(png.name, "../output", "temp"),
        OutputFileReference("missing.png", "", "output"),
        OutputFileReference(png.name, "", "input"),
    ]:
        with pytest.raises(ValueError):
            files.resolve(reference)
    with pytest.raises(ValueError):
        decode_transfer_reference({"filename": "C:/Windows/file.png", "type": "output"})


def test_managed_reveal_uses_registry_roots_and_existing_companion_bytes(
    tmp_path: Path,
) -> None:
    """Resolve opaque external roots without exposing a path or regenerating registered JPEG bytes."""
    root = tmp_path / "external"
    root.mkdir()
    png, jpg = root / "image.png", root / "image.jpg"
    with Image.new("RGB", (5, 7), "blue") as image:
        image.save(png)
        image.save(jpg, quality=64)
    registry = ManagedArtifactRegistry(tmp_path / "state.sqlite")
    managed = registry.register(
        root=root, png=png, jpeg=jpg, saved=True, width=5, height=7
    )
    revealed: list[Path] = []
    files = HostOutputImageFiles(
        registry, {"output": tmp_path / "other"}, Revealer(revealed)
    )
    service = OutputMediaService(OutputSavingPreferences, files)
    reference = decode_transfer_reference(managed.comfy_image())
    assert service.prepare(reference, ImageTransferFormat.JPEG).data == jpg.read_bytes()
    service.reveal(reference)
    assert revealed == [png.resolve()]
    with pytest.raises(ValueError):
        service.reveal(replace(reference, filename="forged.png"))
