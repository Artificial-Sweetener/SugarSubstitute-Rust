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
"""Resolve managed and native images through containment-checked host namespaces."""

from collections.abc import Mapping
from io import BytesIO
from pathlib import Path

from PIL import Image, ImageOps

from ..domain.media import ImageTransferFormat, ImageTransferPayload
from ..domain.preferences import JpegPreferences
from ..domain.transfer import OutputFileReference, OutputTransferChoices
from .artifact_registry import MANAGED_SUBFOLDER_PREFIX, ManagedArtifactRegistry
from .file_revealer import ImageFileRevealer
from .jpeg_encoder import JpegCompanionEncoder
from .transfer_references import ManagedTransferResolver


class HostOutputImageFiles:
    """Own image path qualification and transient encoding without extending the saved artifact registry."""

    def __init__(
        self,
        registry: ManagedArtifactRegistry,
        roots: Mapping[str, Path],
        revealer: ImageFileRevealer,
    ) -> None:
        """Bind explicit Comfy namespaces, managed references and the platform reveal boundary."""
        self._registry = registry
        self._roots = {kind: root.resolve() for kind, root in roots.items()}
        self._choices = ManagedTransferResolver(registry)
        self._revealer = revealer
        self._jpeg = JpegCompanionEncoder()

    @property
    def can_reveal(self) -> bool:
        """Report desktop support without assuming the browser runs on this machine."""
        return self._revealer.available

    def choices(self, reference: OutputFileReference) -> OutputTransferChoices:
        """Validate the requested file before resolving its canonical and optional companion identities."""
        self.resolve(reference)
        choices = self._choices.choices(reference)
        if choices is None:
            raise ValueError("This output image is unavailable.")
        return choices

    def resolve(self, reference: OutputFileReference) -> Path:
        """Reject traversal, junction escape and missing files while delegating opaque managed roots."""
        if reference.subfolder.startswith(MANAGED_SUBFOLDER_PREFIX):
            path = self._registry.resolve(
                reference.subfolder[len(MANAGED_SUBFOLDER_PREFIX) :],
                reference.filename,
                reference.type,
            )
            if path is None:
                raise ValueError("This managed output image is unavailable.")
            return path
        root = self._roots.get(reference.type)
        if root is None:
            raise ValueError("This host image namespace is unavailable.")
        path = (root / reference.subfolder / reference.filename).resolve()
        if (
            not path.is_relative_to(root)
            or not path.is_file()
            or path.suffix.casefold() not in {".png", ".jpg", ".jpeg", ".webp"}
        ):
            raise ValueError(
                "This output image is unavailable or outside its host folder."
            )
        return path

    def rendition(
        self, reference: OutputFileReference, format: ImageTransferFormat, quality: int
    ) -> ImageTransferPayload:
        """Preserve exact bytes for a matching format; encode other renditions only in request memory."""
        path = self.resolve(reference)
        if format is ImageTransferFormat.PNG and path.suffix.casefold() == ".png":
            return ImageTransferPayload(path.read_bytes(), path.name, "image/png")
        if format is ImageTransferFormat.JPEG and path.suffix.casefold() in {
            ".jpg",
            ".jpeg",
        }:
            return ImageTransferPayload(path.read_bytes(), path.name, "image/jpeg")
        if format not in {ImageTransferFormat.PNG, ImageTransferFormat.JPEG}:
            raise ValueError("An explicit image format is required for encoding.")
        with Image.open(path) as original, ImageOps.exif_transpose(original) as image:
            if format is ImageTransferFormat.JPEG:
                data = self._jpeg.encode(image, JpegPreferences(quality=quality))
                return ImageTransferPayload(
                    data, path.with_suffix(".jpg").name, "image/jpeg"
                )
            buffer = BytesIO()
            image.save(buffer, format="PNG")
            return ImageTransferPayload(
                buffer.getvalue(), path.with_suffix(".png").name, "image/png"
            )

    def reveal(self, reference: OutputFileReference) -> None:
        """Open only a currently contained host image, never a client-supplied absolute path."""
        self._revealer.reveal(self.resolve(reference))
