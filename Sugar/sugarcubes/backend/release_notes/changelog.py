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
"""Cut the release notes of a version range out of a semantic-release changelog.

semantic-release heads each version with `#` (a minor release) or `##` (a patch
release), followed by `[x.y.z](compare link)` or, without a link, `x.y.z `. A
version's notes run from its heading to the next version heading; `###` headings
inside belong to the version above them. The notes keep their heading, so the
page shows the version's date and comparison link as the changelog does.
"""

from __future__ import annotations

import re
from dataclasses import dataclass

from ..pack_install.pack_identity import ReleaseVersion

MAX_RELEASES = 64
"""Most versions one answer carries; older versions in the range are counted."""
MAX_NOTES_UNITS = 262_144
"""Longest notes of one version, in UTF-16 code units, as the page bounds them."""
MAX_TOTAL_NOTES_BYTES = 768 * 1024
"""Most UTF-8 notes one answer carries, leaving room under the message bound."""

_VERSION_HEADING = re.compile(
    r"^#{1,2} (?:\[(\d{1,9})\.(\d{1,9})\.(\d{1,9})\]\(|(\d{1,9})\.(\d{1,9})\.(\d{1,9})(?: |$))",
    re.MULTILINE,
)


@dataclass(frozen=True, slots=True)
class ReleaseSection:
    """One version's notes as the changelog holds them."""

    version: ReleaseVersion
    notes: str


@dataclass(frozen=True, slots=True)
class ReleaseNotes:
    """The notes of a version range, newest first, and how many older ones were left out."""

    releases: tuple[ReleaseSection, ...]
    omitted: int


def changelog_sections(changelog: str) -> list[ReleaseSection]:
    """Return every version section of a changelog in the order it lists them."""

    headings = list(_VERSION_HEADING.finditer(changelog))
    sections: list[ReleaseSection] = []
    for index, heading in enumerate(headings):
        end = (
            headings[index + 1].start() if index + 1 < len(headings) else len(changelog)
        )
        parts = [part for part in heading.groups() if part is not None]
        version = ReleaseVersion(int(parts[0]), int(parts[1]), int(parts[2]))
        sections.append(
            ReleaseSection(version, changelog[heading.start() : end].strip())
        )
    return sections


def notes_in_range(
    changelog: str, *, since: ReleaseVersion, through: ReleaseVersion
) -> ReleaseNotes | None:
    """Return the notes of every version after `since` through `through`, newest first.

    A `since` that is not older than `through` asks for `through` alone. Returns
    `None` when the changelog has no section for `through`, which means its notes
    are not published. Versions past the release or byte bounds are counted in
    `omitted` instead of being cut mid-text.
    """

    sections = changelog_sections(changelog)
    if not any(section.version == through for section in sections):
        return None
    if since < through:
        chosen = [s for s in sections if since < s.version <= through]
    else:
        chosen = [s for s in sections if s.version == through]
    chosen.sort(key=lambda section: section.version, reverse=True)
    unique: list[ReleaseSection] = []
    for section in chosen:
        if not unique or unique[-1].version != section.version:
            unique.append(_bounded(section))
    kept: list[ReleaseSection] = []
    total = 0
    for section in unique:
        size = len(section.notes.encode("utf-8"))
        if len(kept) == MAX_RELEASES or (kept and total + size > MAX_TOTAL_NOTES_BYTES):
            break
        kept.append(section)
        total += size
    return ReleaseNotes(releases=tuple(kept), omitted=len(unique) - len(kept))


def _bounded(section: ReleaseSection) -> ReleaseSection:
    """Cut an oversized section at the last line that still fits the page's bound."""

    text = section.notes
    if _utf16_units(text) <= MAX_NOTES_UNITS and len(text.encode("utf-8")) <= (
        MAX_TOTAL_NOTES_BYTES
    ):
        return section
    lines: list[str] = []
    units = 0
    size = 0
    for line in text.split("\n"):
        line_units = _utf16_units(line) + 1
        line_size = len(line.encode("utf-8")) + 1
        if (
            units + line_units > MAX_NOTES_UNITS
            or size + line_size > MAX_TOTAL_NOTES_BYTES
        ):
            break
        lines.append(line)
        units += line_units
        size += line_size
    return ReleaseSection(section.version, "\n".join(lines).rstrip())


def _utf16_units(text: str) -> int:
    """Count UTF-16 code units, the unit the page's schema bounds text in."""

    return len(text.encode("utf-16-le")) // 2


__all__ = [
    "MAX_NOTES_UNITS",
    "MAX_RELEASES",
    "MAX_TOTAL_NOTES_BYTES",
    "ReleaseNotes",
    "ReleaseSection",
    "changelog_sections",
    "notes_in_range",
]
