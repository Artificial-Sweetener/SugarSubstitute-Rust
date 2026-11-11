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
"""Exercise real canonical image files, companion collisions and failure recovery."""

from datetime import datetime
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from PIL import Image
import pytest

from sugarcubes.output_saving.domain.path_pattern import OutputNamingContext
from sugarcubes.output_saving.domain.preferences import JpegPreferences
from sugarcubes.output_saving.infrastructure.image_persistence import (
    ImageFilePersistence,
)


def test_png_and_companion_preserve_pixels_and_recipe_metadata(tmp_path: Path) -> None:
    """Save one canonical recipe PNG and an independently shareable JPEG beside it."""
    persistence = ImageFilePersistence(tmp_path)
    context = OutputNamingContext(
        workflow="Example", source="Result", started_at=datetime(2026, 5, 1), run=7
    )
    image = Image.new("RGB", (12, 9), (10, 20, 30))
    result = persistence.save(
        image,
        pattern="{date}/{run}_{source}",
        context=context,
        metadata={
            "workflow": '{"nodes":[]}',
            "sugar_script": "recipe",
            "prompt": '{"1":{}}',
        },
        jpeg=JpegPreferences(enabled=True),
    )
    assert result.png == tmp_path / "2026-05-01" / "007_result.png"
    assert result.jpeg == result.png.with_suffix(".jpg")
    with Image.open(result.png) as decoded:
        assert decoded.size == (12, 9)
        assert decoded.getpixel((0, 0)) == (10, 20, 30)
        assert decoded.info["workflow"] == '{"nodes":[]}'
        assert decoded.info["sugar_script"] == "recipe"
        assert decoded.info["prompt"] == '{"1":{}}'


def test_companion_collision_does_not_overwrite_either_existing_representation(
    tmp_path: Path,
) -> None:
    """Reserve PNG and JPEG stems together even when only the JPEG already exists."""
    previous = tmp_path / "result.jpg"
    previous.write_bytes(b"existing user image")
    result = ImageFilePersistence(tmp_path).save(
        Image.new("RGB", (2, 2)),
        pattern="result",
        context=OutputNamingContext(
            workflow="", source="", started_at=datetime(2026, 5, 1)
        ),
        metadata={},
        jpeg=JpegPreferences(enabled=True),
    )
    assert result.png.name == "result_2.png"
    assert result.jpeg is not None
    assert result.jpeg.name == "result_2.jpg"
    assert previous.read_bytes() == b"existing user image"
    assert not (tmp_path / "result.png").exists()


def test_canonical_write_failure_removes_only_its_owned_reservation(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Preserve user files and leave no falsely successful artifacts after encoder failure."""
    previous = tmp_path / "result.png"
    previous.write_bytes(b"existing user image")

    def fail_encoding(image: Image.Image, *args: object, **kwargs: object) -> None:
        """Fail at the encoder boundary after a destination was reserved."""
        raise OSError("fixture denies encoding")

    monkeypatch.setattr(Image.Image, "save", fail_encoding)
    with pytest.raises(OSError):
        ImageFilePersistence(tmp_path).save(
            Image.new("RGB", (2, 2)),
            pattern="result",
            context=OutputNamingContext(
                workflow="", source="", started_at=datetime(2026, 5, 1)
            ),
            metadata={},
            jpeg=JpegPreferences(enabled=True),
        )
    assert list(tmp_path.iterdir()) == [previous]
    assert previous.read_bytes() == b"existing user image"


def test_optional_companion_failure_keeps_successful_canonical_png(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Expose successful PNG saving while recording optional companion failure."""
    from sugarcubes.output_saving.infrastructure.jpeg_encoder import (
        JpegCompanionEncoder,
    )

    def fail_companion(
        encoder: JpegCompanionEncoder, image: Image.Image, preferences: JpegPreferences
    ) -> bytes:
        """Model an optional encoding failure without replacing the canonical encoder."""
        raise OSError("fixture denies companion")

    monkeypatch.setattr(JpegCompanionEncoder, "encode", fail_companion)
    result = ImageFilePersistence(tmp_path).save(
        Image.new("RGB", (2, 2)),
        pattern="result",
        context=OutputNamingContext(
            workflow="", source="", started_at=datetime(2026, 5, 1)
        ),
        metadata={},
        jpeg=JpegPreferences(enabled=True),
    )
    assert result.png.is_file()
    assert result.jpeg is None
    assert not result.png.with_suffix(".jpg").exists()


def test_existing_directory_is_not_treated_as_an_overwritable_image(
    tmp_path: Path,
) -> None:
    """Keep collision handling valid for arbitrary preexisting filesystem entries."""
    directory = tmp_path / "result.png"
    directory.mkdir()
    result = ImageFilePersistence(tmp_path).save(
        Image.new("RGB", (2, 2)),
        pattern="result",
        context=OutputNamingContext(
            workflow="", source="", started_at=datetime(2026, 5, 1)
        ),
        metadata={},
        jpeg=JpegPreferences(),
    )
    assert directory.is_dir()
    assert result.png.name == "result_002.png"


def test_filename_literal_dots_are_preserved(tmp_path: Path) -> None:
    """Append the PNG extension without removing meaningful dots in a user's template."""
    result = ImageFilePersistence(tmp_path).save(
        Image.new("RGB", (2, 2)),
        pattern="result.v2",
        context=OutputNamingContext(
            workflow="", source="", started_at=datetime(2026, 5, 1)
        ),
        metadata={},
        jpeg=JpegPreferences(),
    )
    assert result.png.name == "result.v2.png"


def test_concurrent_writes_reserve_distinct_png_and_jpeg_pairs(tmp_path: Path) -> None:
    """Exercise exclusive creation with independent writers racing for the same stem."""

    def save_pair(index: int) -> Path:
        """Write distinguishable real pixels through a separate persistence instance."""
        return (
            ImageFilePersistence(tmp_path)
            .save(
                Image.new("RGB", (2, 2), (index, 0, 0)),
                pattern="result",
                context=OutputNamingContext(
                    workflow="", source="", started_at=datetime(2026, 5, 1)
                ),
                metadata={"index": str(index)},
                jpeg=JpegPreferences(enabled=True),
            )
            .png
        )

    with ThreadPoolExecutor(max_workers=4) as executor:
        paths = list(executor.map(save_pair, range(12)))
    assert len(set(paths)) == 12
    for index, path in enumerate(paths):
        with Image.open(path) as decoded:
            assert decoded.info["index"] == str(index)
            assert decoded.getpixel((0, 0)) == (index, 0, 0)
        assert path.with_suffix(".jpg").is_file()
    assert len(list(tmp_path.iterdir())) == 24
