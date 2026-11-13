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
"""Project Git revision records into unique selectable cube versions."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Mapping, Sequence, TypedDict

from .cube_metadata import normalize_metadata_string


class DuplicateCubeVersionOmission(TypedDict):
    """Describe one exact revision hidden behind a newer matching version."""

    version: str
    selected_revision_ref: str
    omitted_revision_ref: str


@dataclass(frozen=True, slots=True)
class CubeRevisionVersionProjection:
    """Hold selectable revisions and diagnostic-only duplicate omissions."""

    revisions: tuple[dict[str, object], ...]
    omissions: tuple[DuplicateCubeVersionOmission, ...]


def project_unique_cube_versions(
    revisions: Sequence[Mapping[str, object]],
) -> CubeRevisionVersionProjection:
    """Keep the first revision for each version while retaining omission details.

    Revision sources provide the working tree first and Git commits newest first.
    Preserving that order makes the selected revision deterministic without
    deleting or rewriting exact historical commits.
    """

    selected_refs_by_version: dict[str, str] = {}
    projected: list[dict[str, object]] = []
    omissions: list[DuplicateCubeVersionOmission] = []
    for revision in revisions:
        version = normalize_metadata_string(revision.get("version"))
        if not version:
            continue
        selected_ref = selected_refs_by_version.get(version)
        revision_ref = normalize_metadata_string(revision.get("revision_ref"))
        if selected_ref is not None:
            omissions.append(
                {
                    "version": version,
                    "selected_revision_ref": selected_ref,
                    "omitted_revision_ref": revision_ref,
                }
            )
            continue
        selected_refs_by_version[version] = revision_ref
        projected.append(dict(revision))
    return CubeRevisionVersionProjection(tuple(projected), tuple(omissions))
