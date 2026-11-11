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
"""Describe exact encoded artifact identity without path or workflow authority."""

from dataclasses import dataclass
from typing import Literal

from .transfer import OutputFileReference

ArtifactUnavailableCode = Literal[
    "missing",
    "changed",
    "unsupported",
    "inaccessible",
    "invalid_reference",
    "budget",
    "cancelled",
    "ambiguous",
]


class ArtifactUnavailable(ValueError):
    """Preserve a stable per-file diagnostic without exposing private filesystem locations."""

    def __init__(self, code: ArtifactUnavailableCode, message: str) -> None:
        """Bind a machine-readable outcome and an actionable public explanation."""
        super().__init__(message)
        self.code = code


@dataclass(frozen=True, slots=True)
class ArtifactReceipt:
    """Confirm one stable regular file's exact original bytes, independent of embedded metadata."""

    file: OutputFileReference
    digest: str
    bytes: int
    width: int
    height: int
    mime: str
    metadata_embedded: bool
    media: Literal["image", "video"] = "image"
    embedded_prompt_id: str | None = None
    animated: bool = False


@dataclass(frozen=True, slots=True)
class ExpectedArtifact:
    """Name recorded identity; neither a descriptor nor a digest grants access to a root."""

    receipt: ArtifactReceipt
    prompt_id: str | None


def same_artifact(expected: ArtifactReceipt, actual: ArtifactReceipt) -> bool:
    """Require the recorded content and media facts while permitting a relocated descriptor."""
    return (
        expected.digest == actual.digest
        and expected.bytes == actual.bytes
        and expected.width == actual.width
        and expected.height == actual.height
        and expected.mime == actual.mime
        and expected.media == actual.media
        and expected.metadata_embedded == actual.metadata_embedded
    )
