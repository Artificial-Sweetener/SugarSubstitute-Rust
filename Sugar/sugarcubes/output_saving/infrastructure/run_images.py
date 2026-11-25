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
"""Find the saved images of the run a generated image came from, beside that image."""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path

from PIL import Image

from .artifact_registry import ManagedArtifactRegistry

_MAX_SIBLINGS = 512


@dataclass(frozen=True, slots=True)
class RunImage:
    """One saved image of a run: its Comfy image reference and the output it came from."""

    image: Mapping[str, object]
    source_node_id: str
    source_label: str
    image_index: int


@dataclass(frozen=True, slots=True)
class DroppedImage:
    """The image a person brought to the page: its name, size, content hash and run."""

    filename: str
    size: int
    sha256: str
    run_id: str


class OutputRunImages:
    """Find a dropped image among the saved outputs and list its run's images in that folder.

    A run's images are saved together in one folder and each records its run in the
    `sugarcubes_output` chunk, so the dropped image's own folder holds the run, as the Python
    SugarSubstitute app found them. The dropped image is matched by content, never by name alone.
    """

    def __init__(self, registry: ManagedArtifactRegistry) -> None:
        """Bind the registry of saved images."""
        self._registry = registry

    def find(self, dropped: DroppedImage) -> tuple[RunImage, ...]:
        """Return the run's saved images in name order, or none when the image is not saved here."""
        found = next(
            (
                path
                for _identity, path in self._registry.named(dropped.filename)
                if _same_file(path, dropped)
            ),
            None,
        )
        if found is None:
            return ()
        images: list[tuple[str, RunImage]] = []
        for identity, path in self._registry.beside(found)[:_MAX_SIBLINGS]:
            record = _output_record(path)
            if record is None or record.get("run_id") != dropped.run_id:
                continue
            reference = self._registry.reference(identity, path.name, "output")
            if reference is None:
                continue
            images.append(
                (
                    path.name,
                    RunImage(
                        image=reference.comfy_image(),
                        source_node_id=str(record.get("source_node_id") or ""),
                        source_label=str(record.get("source_label") or ""),
                        image_index=_index(record.get("image_index")),
                    ),
                )
            )
        return tuple(image for _name, image in sorted(images, key=lambda item: item[0]))


def read_dropped_image(value: object) -> DroppedImage:
    """Validate the page's description of a dropped image."""
    if not isinstance(value, Mapping):
        raise ValueError("The dropped image must be described by an object.")
    filename, run_id, sha256 = (
        value.get(key) for key in ("filename", "run_id", "sha256")
    )
    size = value.get("size")
    if (
        not isinstance(filename, str)
        or not filename.strip()
        or "/" in filename
        or "\\" in filename
        or not isinstance(run_id, str)
        or not run_id.strip()
        or not isinstance(sha256, str)
        or len(sha256) != 64
        or isinstance(size, bool)
        or not isinstance(size, int)
        or size < 1
    ):
        raise ValueError("The dropped image description is invalid.")
    return DroppedImage(filename.strip(), size, sha256.lower(), run_id.strip())


def _same_file(path: Path, dropped: DroppedImage) -> bool:
    """Whether a saved file holds exactly the dropped bytes."""
    try:
        if path.stat().st_size != dropped.size:
            return False
        digest = hashlib.sha256()
        with path.open("rb") as stream:
            for block in iter(lambda: stream.read(1 << 20), b""):
                digest.update(block)
        return digest.hexdigest() == dropped.sha256
    except OSError:
        return False


def _output_record(path: Path) -> Mapping[str, object] | None:
    """Read a saved image's `sugarcubes_output` record from its leading text chunks."""
    try:
        with Image.open(path) as image:
            raw = image.info.get("sugarcubes_output")
    except (OSError, ValueError):
        return None
    if not isinstance(raw, str):
        return None
    try:
        record: object = json.loads(raw)
    except ValueError:
        return None
    return record if isinstance(record, Mapping) else None


def _index(value: object) -> int:
    """Read a recorded image index, defaulting to the first."""
    return value if isinstance(value, int) and not isinstance(value, bool) else 1


__all__ = ["DroppedImage", "OutputRunImages", "RunImage", "read_dropped_image"]
