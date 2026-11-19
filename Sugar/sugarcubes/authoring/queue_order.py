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
"""Decide the run order of jobs that are still waiting."""

from __future__ import annotations

from collections.abc import Sequence


def reordered_places(
    waiting: Sequence[tuple[float, str]], order: Sequence[str]
) -> dict[str, float]:
    """Return the place each waiting job takes so the named jobs run in ``order``.

    ``waiting`` holds each waiting job's place and identifier; a lower place runs sooner.
    The jobs named in ``order`` exchange the places they already hold among themselves, so
    a job that is not named, or one queued while the request was on its way, keeps its
    place and nothing is given a place no job held. Names that are no longer waiting,
    because their job started or was cancelled, are ignored, as are repeated names.

    Returns only the jobs whose place changes.
    """

    held = {job: place for place, job in waiting}
    named: list[str] = []
    for job in order:
        if job in held and job not in named:
            named.append(job)
    places = sorted(held[job] for job in named)
    return {
        job: place
        for job, place in zip(named, places, strict=True)
        if held[job] != place
    }


__all__ = ["reordered_places"]
