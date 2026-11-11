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
"""Adapt validated saving tickets into Comfy UI references and neutral cube artifacts."""

from __future__ import annotations

from ...runtime.cube_output_events import CubeOutputArtifact

from .image_tensors import is_image_tensor_batch, iter_tensor_images
from .managed_image_writer import ManagedImageWriter
from .ticket_journal import SavingTicketJournal


class RuntimeSavingImages:
    """Own per-node image materialization without retaining tensors after the sink returns."""

    def __init__(
        self, journal: SavingTicketJournal, writer: ManagedImageWriter
    ) -> None:
        """Bind the authoritative queued ticket and image-write boundaries."""
        self._journal = journal
        self._writer = writer

    def save(
        self, value: object, ticket_id: str
    ) -> tuple[dict[str, object], tuple[CubeOutputArtifact, ...]]:
        """Materialize real images while leaving non-image cube resources available to execution."""
        ticket = self._journal.read_ticket(ticket_id)
        if not is_image_tensor_batch(value):
            return {}, ()
        images: list[dict[str, object]] = []
        artifacts: list[CubeOutputArtifact] = []
        for image in iter_tensor_images(value):
            ref = self._writer.write(image, ticket)
            images.append(ref.comfy_image())
            artifacts.append(
                CubeOutputArtifact(
                    filename=ref.filename,
                    subfolder=ref.subfolder,
                    type=ref.type,
                    media_kind="image",
                    mime_type="image/png",
                    width=ref.width,
                    height=ref.height,
                )
            )
        return {"images": images}, tuple(artifacts)
