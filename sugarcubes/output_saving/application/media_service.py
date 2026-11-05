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
"""Sequence validated image renditions, file reveal and Photoshop through explicit platform ports."""

from collections.abc import Callable, Sequence
from typing import NamedTuple, Protocol

from ..domain.media import (
    ImageTransferFormat,
    ImageTransferPayload,
    preferred_copy_format,
)
from ..domain.preferences import OutputSavingPreferences
from ..domain.transfer import OutputFileReference, OutputTransferChoices


class OutputImageFiles(Protocol):
    """Materialize only validated host image identities through the storage adapter."""

    def choices(self, reference: OutputFileReference) -> OutputTransferChoices:
        """Revalidate canonical and companion availability before using their bytes."""

    def rendition(
        self, reference: OutputFileReference, format: ImageTransferFormat, quality: int
    ) -> ImageTransferPayload:
        """Encode one full-resolution request without saving another output."""

    def reveal(self, reference: OutputFileReference) -> None:
        """Reveal exactly one validated host file through the desktop adapter."""

    @property
    def can_reveal(self) -> bool:
        """Report installed desktop support independently of request locality."""

    def open_in_photoshop(
        self, document: str, layers: Sequence[tuple[OutputFileReference, str]]
    ) -> None:
        """Open validated still images as named layers of one new Photoshop document."""

    @property
    def can_open_in_photoshop(self) -> bool:
        """Report an installed Photoshop independently of request locality."""


class OutputMediaProfile(NamedTuple):
    """Publish copy preference and installed desktop actions without opening files."""

    copy_format: ImageTransferFormat
    reveal: bool
    photoshop: bool


class OutputMediaService:
    """Keep explicit and preferred format selection shared by web and native canvases."""

    def __init__(
        self,
        preferences: Callable[[], OutputSavingPreferences],
        files: OutputImageFiles,
    ) -> None:
        """Bind saving preference authority and the validated image-file boundary."""
        self._preferences = preferences
        self._files = files

    def profile(self) -> OutputMediaProfile:
        """Publish current primary copy preference and desktop availability without opening files."""
        return OutputMediaProfile(
            preferred_copy_format(self._preferences()),
            self._files.can_reveal,
            self._files.can_open_in_photoshop,
        )

    def prepare(
        self, reference: OutputFileReference, format: ImageTransferFormat
    ) -> ImageTransferPayload:
        """Reuse an available JPEG companion or encode at the retained quality for this request."""
        preferences = self._preferences()
        desired = (
            preferred_copy_format(preferences)
            if format is ImageTransferFormat.PREFERRED
            else format
        )
        choices = self._files.choices(reference)
        source = (
            choices.companion
            if desired is ImageTransferFormat.JPEG and choices.companion
            else choices.canonical
        )
        return self._files.rendition(source.file, desired, preferences.jpeg.quality)

    def reveal(self, reference: OutputFileReference) -> None:
        """Delegate a qualified local request without granting callers arbitrary path access."""
        self._files.reveal(reference)

    def open_in_photoshop(
        self, document: str, layers: Sequence[tuple[OutputFileReference, str]]
    ) -> None:
        """Delegate a qualified local request; callers name outputs, never file paths."""
        self._files.open_in_photoshop(document, layers)
