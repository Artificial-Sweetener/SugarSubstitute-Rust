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
"""Choose output representations independently of a canvas, clipboard or operating system."""

from dataclasses import dataclass

from .preferences import OutputSavingPreferences, TransferFormat


@dataclass(frozen=True, slots=True)
class OutputFileReference:
    """Address a validated host image without exposing an absolute filesystem path."""

    filename: str
    subfolder: str
    type: str


@dataclass(frozen=True, slots=True)
class TransferRepresentation:
    """Describe the exact file and MIME type selected for a drag, copy or native handoff."""

    file: OutputFileReference
    mime_type: str


@dataclass(frozen=True, slots=True)
class OutputTransferChoices:
    """Keep available representations separate from user preference and platform capabilities."""

    canonical: TransferRepresentation
    companion: TransferRepresentation | None = None


def choose_output_transfer(
    choices: OutputTransferChoices, preferences: OutputSavingPreferences
) -> TransferRepresentation:
    """Prefer an existing companion when enabled, retaining the canonical file for every fallback."""
    if (
        preferences.effective_transfer_format is TransferFormat.COMPANION_JPEG
        and choices.companion is not None
    ):
        return choices.companion
    return choices.canonical
