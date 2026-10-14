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
"""Validate portable image references and resolve managed companions through the artifact registry."""

from collections.abc import Mapping
from dataclasses import asdict

from ..domain.transfer import (
    OutputFileReference,
    OutputTransferChoices,
    TransferRepresentation,
)
from .artifact_registry import MANAGED_SUBFOLDER_PREFIX, ManagedArtifactRegistry


def decode_transfer_reference(value: object) -> OutputFileReference:
    """Reject absolute paths, traversal and non-image references before they cross the application boundary."""
    if not isinstance(value, Mapping):
        raise ValueError("An output image reference is required.")
    filename, subfolder, kind = (
        value.get("filename"),
        value.get("subfolder", ""),
        value.get("type"),
    )
    if (
        not isinstance(filename, str)
        or not filename
        or any(char in filename for char in "/\\:\x00")
        or filename in {".", ".."}
    ):
        raise ValueError("The output image filename is invalid.")
    if not isinstance(subfolder, str) or any(char in subfolder for char in ":\x00"):
        raise ValueError("The output image folder is invalid.")
    folder = subfolder.replace("\\", "/")
    if (
        folder.startswith("/")
        or any(part in {".", ".."} for part in folder.split("/"))
        or not isinstance(kind, str)
        or kind not in {"input", "output", "temp"}
    ):
        raise ValueError(
            "The output image reference is outside the host image namespace."
        )
    _mime(filename)
    return OutputFileReference(filename, folder, kind)


def encode_transfer(selection: TransferRepresentation) -> dict[str, object]:
    """Publish one portable file selection for every canvas adapter."""
    return {"file": asdict(selection.file), "mime_type": selection.mime_type}


class ManagedTransferResolver:
    """Resolve companion availability without duplicating registry containment or persistence policy."""

    def __init__(self, registry: ManagedArtifactRegistry) -> None:
        """Bind the authoritative opaque artifact lookup."""
        self._registry = registry

    def choices(self, reference: OutputFileReference) -> OutputTransferChoices | None:
        """Preserve ordinary native images and select only registered, existing managed representations."""
        if not reference.subfolder.startswith(MANAGED_SUBFOLDER_PREFIX):
            return OutputTransferChoices(
                TransferRepresentation(reference, _mime(reference.filename))
            )
        identity = reference.subfolder[len(MANAGED_SUBFOLDER_PREFIX) :]
        image = self._registry.reference(identity, reference.filename, reference.type)
        if image is None:
            return None
        canonical = TransferRepresentation(
            OutputFileReference(image.filename, image.subfolder, image.type),
            "image/png",
        )
        companion = (
            TransferRepresentation(
                OutputFileReference(
                    image.companion_filename, image.subfolder, image.type
                ),
                "image/jpeg",
            )
            if image.companion_filename is not None
            else None
        )
        return OutputTransferChoices(canonical, companion)


def _mime(filename: str) -> str:
    """Admit supported still-image formats while leaving animated and video transfer to their own adapters."""
    suffix = filename.rsplit(".", 1)[-1].lower()
    if suffix not in {"png", "jpg", "jpeg", "webp"}:
        raise ValueError("This output is not a supported still image.")
    return {
        "png": "image/png",
        "jpg": "image/jpeg",
        "jpeg": "image/jpeg",
        "webp": "image/webp",
    }[suffix]
