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
"""Coordinate current output transfer preferences through a validated artifact boundary."""

from typing import Protocol
from collections.abc import Callable

from ..domain.preferences import OutputSavingPreferences
from ..domain.transfer import (
    OutputFileReference,
    OutputTransferChoices,
    TransferRepresentation,
    choose_output_transfer,
)


class OutputTransferResolver(Protocol):
    """Resolve only available host-owned image representations."""

    def choices(self, reference: OutputFileReference) -> OutputTransferChoices | None:
        """Return available canonical and companion references without retaining image pixels."""


class OutputTransferService:
    """Keep transfer preference authoritative for web, native and detached consumers alike."""

    def __init__(
        self,
        preferences: Callable[[], OutputSavingPreferences],
        resolver: OutputTransferResolver,
    ) -> None:
        """Bind shared preferences and the validated artifact owner rather than any renderer."""
        self._preferences = preferences
        self._resolver = resolver

    def choose(self, reference: OutputFileReference) -> TransferRepresentation:
        """Revalidate current availability and apply the user's current transfer choice."""
        choices = self._resolver.choices(reference)
        if choices is None:
            raise ValueError("That output image is unavailable for transfer.")
        return choose_output_transfer(choices, self._preferences())
