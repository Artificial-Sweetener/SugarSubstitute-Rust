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
"""Describe transient image renditions independently of storage and desktop transport."""

from dataclasses import dataclass
from enum import Enum
from typing import Literal

from .preferences import OutputSavingPreferences, TransferFormat

ImageMimeType = Literal["image/png", "image/jpeg", "image/webp"]


class ImageTransferFormat(str, Enum):
    """Distinguish an explicit clipboard rendition from the user's preferred transfer format."""

    PREFERRED = "preferred"
    PNG = "png"
    JPEG = "jpeg"


@dataclass(frozen=True, slots=True)
class ImageTransferPayload:
    """Hold one request's full-resolution bytes without creating a persistent companion."""

    data: bytes
    filename: str
    mime_type: ImageMimeType


def preferred_copy_format(preferences: OutputSavingPreferences) -> ImageTransferFormat:
    """Apply the same enabled drag-format preference to the primary Copy command."""
    return (
        ImageTransferFormat.JPEG
        if preferences.effective_transfer_format is TransferFormat.COMPANION_JPEG
        else ImageTransferFormat.PNG
    )
