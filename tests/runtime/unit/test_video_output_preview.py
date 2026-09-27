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

"""Verify video values become Comfy-addressable cube output artifacts."""

from __future__ import annotations

import io
import sys
from pathlib import Path
from types import ModuleType

import pytest

from sugarcubes.runtime.video_output_preview import build_video_output_preview


class FileVideo:
    """Expose a file-backed Comfy video-compatible test value."""

    def __init__(
        self,
        source: str,
        *,
        trim_window: tuple[float, float] = (0.0, 0.0),
    ) -> None:
        """Store the source and active trim used by the adapter."""

        self.source = source
        self.trim_window = trim_window
        self.saved_paths: list[Path] = []

    def get_stream_source(self) -> str:
        """Return the file source without materializing frames."""

        return self.source

    def get_active_trim_window(self) -> tuple[float, float]:
        """Return the active trim contract."""

        return self.trim_window

    def get_dimensions(self) -> tuple[int, int]:
        """Return stable media dimensions."""

        return 320, 180

    def get_duration(self) -> float:
        """Return a stable media duration."""

        return 1.25

    def save_to(self, path: str) -> None:
        """Record and materialize a trimmed video artifact."""

        destination = Path(path)
        destination.write_bytes(b"trimmed-video")
        self.saved_paths.append(destination)


class BufferedVideo:
    """Expose a buffer-backed Comfy video-compatible test value."""

    def get_stream_source(self) -> io.BytesIO:
        """Return encoded video bytes."""

        return io.BytesIO(b"buffered-video")

    def get_active_trim_window(self) -> tuple[float, float]:
        """Return the untrimmed contract."""

        return 0.0, 0.0

    def get_dimensions(self) -> tuple[int, int]:
        """Return stable media dimensions."""

        return 640, 360

    def get_duration(self) -> float:
        """Return a stable media duration."""

        return 2.5

    def save_to(self, path: str) -> None:
        """Reject fallback encoding because the buffer is already encoded."""

        raise AssertionError(path)


@pytest.fixture
def comfy_folders(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> dict[str, Path]:
    """Install deterministic Comfy folder roots for adapter tests."""

    roots = {
        "input": tmp_path / "input",
        "output": tmp_path / "output",
        "temp": tmp_path / "temp",
    }
    for root in roots.values():
        root.mkdir()
    folder_paths = ModuleType("folder_paths")
    folder_paths.get_input_directory = lambda: str(roots["input"])  # type: ignore[attr-defined]
    folder_paths.get_output_directory = lambda: str(roots["output"])  # type: ignore[attr-defined]
    folder_paths.get_temp_directory = lambda: str(roots["temp"])  # type: ignore[attr-defined]
    monkeypatch.setitem(sys.modules, "folder_paths", folder_paths)
    return roots


def test_file_backed_video_reuses_comfy_visible_source(
    comfy_folders: dict[str, Path],
) -> None:
    """Reference an untrimmed Comfy input without copying or re-encoding it."""

    source = comfy_folders["input"] / "clips" / "sample.mp4"
    source.parent.mkdir()
    source.write_bytes(b"source-video")

    ui_payload, artifacts = build_video_output_preview(FileVideo(str(source)))

    assert ui_payload == {
        "images": [
            {
                "filename": "sample.mp4",
                "subfolder": "clips",
                "type": "input",
            }
        ],
        "animated": (True,),
    }
    assert len(artifacts) == 1
    artifact = artifacts[0]
    assert artifact.filename == "sample.mp4"
    assert artifact.subfolder == "clips"
    assert artifact.type == "input"
    assert artifact.media_kind == "video"
    assert artifact.mime_type == "video/mp4"
    assert artifact.width == 320
    assert artifact.height == 180
    assert artifact.duration_seconds == 1.25


def test_buffer_backed_video_materializes_in_comfy_temp(
    comfy_folders: dict[str, Path],
) -> None:
    """Persist encoded buffers so Substitute can fetch them through Comfy."""

    ui_payload, artifacts = build_video_output_preview(BufferedVideo())

    assert len(artifacts) == 1
    artifact = artifacts[0]
    assert artifact.type == "temp"
    assert artifact.subfolder == ""
    assert artifact.filename.endswith(".mp4")
    assert (comfy_folders["temp"] / artifact.filename).read_bytes() == b"buffered-video"
    assert ui_payload["images"] == [
        {
            "filename": artifact.filename,
            "subfolder": "",
            "type": "temp",
        }
    ]


def test_trimmed_file_video_materializes_active_window(
    comfy_folders: dict[str, Path],
) -> None:
    """Avoid exposing the untrimmed source when the value represents a slice."""

    source = comfy_folders["input"] / "sample.mp4"
    source.write_bytes(b"untrimmed-video")
    video = FileVideo(str(source), trim_window=(0.5, 1.0))

    _, artifacts = build_video_output_preview(video)

    assert len(video.saved_paths) == 1
    assert video.saved_paths[0].parent == comfy_folders["temp"]
    assert artifacts[0].type == "temp"
    assert video.saved_paths[0].read_bytes() == b"trimmed-video"


def test_non_video_value_has_no_video_preview(
    comfy_folders: dict[str, Path],
) -> None:
    """Leave unrelated runtime values to the existing generic output path."""

    assert build_video_output_preview(object()) == ({}, ())
