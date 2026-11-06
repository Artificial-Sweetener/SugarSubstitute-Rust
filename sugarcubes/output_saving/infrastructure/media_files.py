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

from collections.abc import Mapping, Sequence
from io import BytesIO
from pathlib import Path

from PIL import Image, ImageOps

from ..domain.media import ImageTransferFormat, ImageTransferPayload
from ..domain.preferences import JpegPreferences
from ..domain.transfer import OutputFileReference, OutputTransferChoices
from .artifact_registry import MANAGED_SUBFOLDER_PREFIX, ManagedArtifactRegistry
from .settings_paths import resolve_output_root
from .file_revealer import ImageFileRevealer
from .contained_files import contained_file
from .jpeg_encoder import JpegCompanionEncoder
from .photoshop_opener import PhotoshopImageOpener, PhotoshopLayer
from .transfer_references import ManagedTransferResolver


class HostOutputImageFiles:
    """Own image path qualification and transient encoding without extending the saved artifact registry."""

    def __init__(
        self,
        registry: ManagedArtifactRegistry,
        roots: Mapping[str, Path],
        revealer: ImageFileRevealer,
        photoshop: PhotoshopImageOpener | None = None,
    ) -> None:
        """Bind explicit Comfy namespaces, managed references and the desktop action boundaries."""
        self._registry = registry
        self._roots = {kind: resolve_output_root(root) for kind, root in roots.items()}
        self._choices = ManagedTransferResolver(registry)
        self._revealer = revealer
        self._photoshop = photoshop
        self._jpeg = JpegCompanionEncoder()

    @property
    def can_reveal(self) -> bool:
        """Report desktop support without assuming the browser runs on this machine."""
        return self._revealer.available

    @property
    def can_open_in_photoshop(self) -> bool:
        """Report an installed Photoshop without assuming the browser runs on this machine."""
        return self._photoshop is not None and self._photoshop.available

    def choices(self, reference: OutputFileReference) -> OutputTransferChoices:
        """Validate the requested file before resolving its canonical and optional companion identities."""
        self.resolve(reference)
        choices = self._choices.choices(reference)
        if choices is None:
            raise ValueError("This output image is unavailable.")
        return choices

    def resolve(self, reference: OutputFileReference) -> Path:
        """Resolve a contained file only when it is a supported still image."""
        path = self._contained(reference)
        if path.suffix.casefold() not in {".png", ".jpg", ".jpeg", ".webp"}:
            raise ValueError(
                "This output image is unavailable or outside its host folder."
            )
        return path

    def _contained(self, reference: OutputFileReference) -> Path:
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
        try:
            return contained_file(root, root / reference.subfolder / reference.filename)
        except OSError as error:
            raise ValueError(
                "This output image is unavailable or outside its host folder."
            ) from error

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

    def preview(
        self, reference: OutputFileReference, longest_side: int
    ) -> ImageTransferPayload:
        """Decode at reduced scale where the codec allows and encode a small WebP in request memory."""
        path = self.resolve(reference)
        with Image.open(path) as original:
            # JPEG decodes directly at a power-of-two reduction, avoiding a full-size bitmap.
            original.draft("RGB", (longest_side, longest_side))
            with ImageOps.exif_transpose(original) as oriented:
                image = oriented.convert(
                    "RGBA" if "A" in oriented.getbands() else "RGB"
                )
        image.thumbnail(
            (longest_side, longest_side), Image.Resampling.BILINEAR, reducing_gap=2.0
        )
        buffer = BytesIO()
        image.save(buffer, format="WEBP", quality=70, method=0)
        return ImageTransferPayload(
            buffer.getvalue(), f"{path.stem}.preview.webp", "image/webp"
        )

    def reveal(self, reference: OutputFileReference) -> None:
        """Select a currently contained output of any media kind, never a client-supplied absolute path."""
        self._revealer.reveal(self._contained(reference))

    def open_in_photoshop(
        self, document: str, layers: Sequence[tuple[OutputFileReference, str]]
    ) -> None:
        """Open currently contained still images, never client-supplied absolute paths."""
        if self._photoshop is None:
            raise ValueError("Photoshop is unavailable on this computer.")
        self._photoshop.open(
            document,
            [
                PhotoshopLayer(self.resolve(reference), name)
                for reference, name in layers
            ],
        )
