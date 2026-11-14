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
"""Encode release notes as the JSON the page's What's new view reads (schema 1).

`GET /sugarcubes/release-notes` answers `{schema, product, currentVersion}`.
`GET /sugarcubes/release-notes/notes?since=x.y.z` answers
`{schema, releases: [{version, notes}], omitted}` with the newest version first,
or `{schema, failure}` where failure is `unavailable` or `not-published`.
"""

from __future__ import annotations

from .changelog import ReleaseNotes
from .service import PRODUCT, NotesUnavailable

RELEASE_NOTES_SCHEMA = 1


def version_to_wire(current_version: str) -> dict[str, object]:
    """Return the product and running version the page labels its notes with."""

    return {
        "schema": RELEASE_NOTES_SCHEMA,
        "product": PRODUCT,
        "currentVersion": current_version,
    }


def notes_to_wire(outcome: ReleaseNotes | NotesUnavailable) -> dict[str, object]:
    """Return the JSON object for one notes read."""

    if isinstance(outcome, NotesUnavailable):
        return {"schema": RELEASE_NOTES_SCHEMA, "failure": outcome.reason.value}
    return {
        "schema": RELEASE_NOTES_SCHEMA,
        "releases": [
            {"version": str(section.version), "notes": section.notes}
            for section in outcome.releases
        ],
        "omitted": outcome.omitted,
    }


__all__ = ["RELEASE_NOTES_SCHEMA", "notes_to_wire", "version_to_wire"]
