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
"""Materialize one image through a captured saving ticket and return its native Comfy reference."""

from __future__ import annotations

from pathlib import Path
import json

from PIL import Image

from ..domain.preferences import JpegPreferences
from ..domain.run_plan import SavingTicket
from .artifact_registry import ManagedArtifactRegistry, ManagedImageReference
from .image_persistence import ImageFilePersistence
from .naming_numbers import OutputNamingNumbers


class ManagedImageWriter:
    """Coordinate image numbering, canonical or transient writes and durable reference registration."""

    def __init__(
        self,
        *,
        numbers: OutputNamingNumbers,
        artifacts: ManagedArtifactRegistry,
        temporary_root: Path,
    ) -> None:
        """Bind focused filesystem owners without retaining any generated image pixels."""
        self._numbers = numbers
        self._artifacts = artifacts
        self._temporary_root = temporary_root

    def write(self, image: Image.Image, ticket: SavingTicket) -> ManagedImageReference:
        """Use the immutable queued policy regardless of current settings or visible workflow."""
        context = self._numbers.context(ticket, width=image.width, height=image.height)
        saved = ticket.source.persist
        root = Path(ticket.run.root) if saved else self._temporary_root
        pattern = (
            ticket.run.preferences.organization.path_pattern
            if saved
            else f"SugarCubes_preview_{ticket.ticket_id}_{context.index}"
        )
        jpeg = ticket.run.preferences.jpeg if saved else JpegPreferences()
        files = ImageFilePersistence(root).save(
            image,
            pattern=pattern,
            context=context,
            metadata=self._metadata(ticket, context.index, context.run),
            jpeg=jpeg,
        )
        return self._artifacts.register(
            root=root,
            png=files.png,
            jpeg=files.jpeg,
            saved=saved,
            width=image.width,
            height=image.height,
        )

    @staticmethod
    def _metadata(
        ticket: SavingTicket, index: int, number: int | None
    ) -> dict[str, str]:
        """Embed exact source and run identity for restoration without relying on filename parsing."""
        if not ticket.run.metadata_enabled:
            return {}
        metadata = dict(ticket.run.metadata)
        metadata["sugarcubes_output"] = json.dumps(
            {
                "schema_version": 1,
                "run_id": ticket.run.run_id,
                "source_node_id": ticket.source.node_id,
                "source_label": ticket.source.label,
                "cube_instance_id": ticket.source.cube_id,
                "cube_number": ticket.source.cube_number,
                "image_index": index,
                "run_number": number,
                "seed": ticket.source.seed,
            },
            ensure_ascii=False,
        )
        return metadata
