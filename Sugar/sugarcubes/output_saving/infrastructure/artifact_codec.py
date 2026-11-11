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
"""Validate bounded artifact HTTP values independently of filesystem and session membership."""

from collections.abc import Mapping
from typing import Literal
import re
import uuid

from ..domain.artifact import ArtifactReceipt, ArtifactUnavailable, ExpectedArtifact
from ..domain.transfer import OutputFileReference
from .artifact_registry import MANAGED_SUBFOLDER_PREFIX
from .transfer_references import decode_output_reference

_INVALID_COMPONENT = re.compile(r'[<>:"/\\|?*\x00-\x1f\x7f]')
_DEVICE = re.compile(r"^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)", re.IGNORECASE)


def _component(value: str) -> bool:
    """Mirror portable path semantics before granting a descriptor access to approved roots."""
    return (
        bool(value)
        and value not in {".", ".."}
        and not _INVALID_COMPONENT.search(value)
        and not value.endswith((".", " "))
        and not _DEVICE.match(value)
    )


def _record(value: object, fields: set[str]) -> Mapping[str, object]:
    """Accept an exact JSON object, refusing unknown protocol versions or accidental authority fields."""
    if (
        not isinstance(value, dict)
        or set(value) != fields
        or not all(isinstance(key, str) for key in value)
    ):
        raise ArtifactUnavailable(
            "invalid_reference", "The output inspection request is invalid."
        )
    return value


def artifact_file(value: object, *, session: bool) -> OutputFileReference:
    """Validate relative descriptors and require a consistent opaque managed identity."""
    fields = (
        {"storage", "filename", "subfolder", "managedArtifact"}
        if session
        else {"type", "filename", "subfolder"}
    )
    data = _record(value, fields)
    reference = decode_output_reference(
        {
            "type": data.get("storage") if session else data.get("type"),
            "filename": data.get("filename"),
            "subfolder": data.get("subfolder"),
        }
    )
    if (
        len(reference.filename.encode("utf-8")) > 255
        or len(reference.subfolder.encode("utf-8")) > 4096
        or not _component(reference.filename)
        or (
            reference.subfolder
            and not all(_component(part) for part in reference.subfolder.split("/"))
        )
    ):
        raise ArtifactUnavailable(
            "invalid_reference", "The output file descriptor is invalid."
        )
    managed = managed_identity(reference)
    if session and data.get("managedArtifact") != managed:
        raise ArtifactUnavailable(
            "invalid_reference",
            "The managed output identity does not match its descriptor.",
        )
    return reference


def managed_identity(file: OutputFileReference) -> str | None:
    """Keep the opaque registry token consistent with the existing managed /view namespace."""
    if not file.subfolder.startswith(MANAGED_SUBFOLDER_PREFIX):
        return None
    identity = file.subfolder[len(MANAGED_SUBFOLDER_PREFIX) :]
    try:
        if str(uuid.UUID(identity)) != identity or uuid.UUID(identity).int == 0:
            raise ValueError("Noncanonical artifact identity")
    except ValueError as error:
        raise ArtifactUnavailable(
            "invalid_reference", "The managed output identity is invalid."
        ) from error
    return identity


def receipt_request(value: object) -> OutputFileReference:
    """Admit only a file descriptor for initial current-byte confirmation."""
    return artifact_file(_record(value, {"file"})["file"], session=False)


def expected_request(value: object) -> ExpectedArtifact:
    """Project artifact-relevant facts without duplicating portable workflow or source validation."""
    data = _record(
        _record(value, {"expected"})["expected"],
        {
            "file",
            "digest",
            "bytes",
            "width",
            "height",
            "media",
            "mime",
            "metadataEmbedded",
            "promptId",
        },
    )
    file = artifact_file(data["file"], session=True)
    digest, size, width, height, mime, embedded, prompt = (
        data[key]
        for key in (
            "digest",
            "bytes",
            "width",
            "height",
            "mime",
            "metadataEmbedded",
            "promptId",
        )
    )
    if (
        not isinstance(digest, str)
        or re.fullmatch("[0-9a-f]{64}", digest) is None
        or type(size) is not int
        or not 1 <= size <= 8589934592
        or type(width) is not int
        or not 1 <= width <= 131072
        or type(height) is not int
        or not 1 <= height <= 131072
        or not isinstance(mime, str)
        or data["media"] not in {"image", "video"}
        or type(embedded) is not bool
        or not (
            prompt is None
            or isinstance(prompt, str)
            and 1 <= len(prompt.encode("utf-8")) <= 256
        )
    ):
        raise ArtifactUnavailable(
            "invalid_reference", "The recorded output identity is invalid."
        )
    if mime not in {
        "image/png",
        "image/jpeg",
        "image/webp",
        "image/gif",
        "video/mp4",
        "video/webm",
    }:
        raise ArtifactUnavailable(
            "unsupported", "This output media format cannot be restored."
        )
    media: Literal["image", "video"] = "video" if data["media"] == "video" else "image"
    if mime.startswith("video/") != (media == "video"):
        raise ArtifactUnavailable(
            "invalid_reference", "The output media kind is inconsistent."
        )
    return ExpectedArtifact(
        ArtifactReceipt(file, digest, size, width, height, mime, embedded, media=media),
        prompt,
    )


def encode_receipt(receipt: ArtifactReceipt) -> dict[str, object]:
    """Publish confirmed encoded identity and a relative current descriptor without filesystem paths."""
    return {
        "available": True,
        "receipt": {
            "file": {
                "storage": receipt.file.type,
                "filename": receipt.file.filename,
                "subfolder": receipt.file.subfolder,
                "managedArtifact": managed_identity(receipt.file),
            },
            "digest": receipt.digest,
            "bytes": receipt.bytes,
            "width": receipt.width,
            "height": receipt.height,
            "media": receipt.media,
            "mime": receipt.mime,
            "metadataEmbedded": receipt.metadata_embedded,
            "animated": receipt.animated,
        },
    }
