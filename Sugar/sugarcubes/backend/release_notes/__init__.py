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
"""Show SugarCubes' published release notes in the page's What's new view."""

from __future__ import annotations

from .changelog import ReleaseNotes, ReleaseSection, notes_in_range
from .service import (
    NotesFailure,
    NotesUnavailable,
    PRODUCT,
    ReleaseNotesService,
    build_release_notes_service,
)
from .wire import RELEASE_NOTES_SCHEMA, notes_to_wire, version_to_wire

__all__ = [
    "NotesFailure",
    "NotesUnavailable",
    "PRODUCT",
    "RELEASE_NOTES_SCHEMA",
    "ReleaseNotes",
    "ReleaseNotesService",
    "ReleaseSection",
    "build_release_notes_service",
    "notes_in_range",
    "notes_to_wire",
    "version_to_wire",
]
