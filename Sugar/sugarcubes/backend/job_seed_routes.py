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
"""Adapt reading one job's seeds to JSON."""

from __future__ import annotations

import json
from collections.abc import Mapping
from typing import Any

from ..authoring.job_seeds import JobSeed, read_job_seeds, read_queued_job_seeds
from .responses import BackendError, json_error_from_exception, json_success
from .validation import parse_json_body

_MAX_HISTORY_BYTES = 32_000_000


def _parsed(body: Mapping[str, Any], field: str) -> object:
    """Parse one Comfy reply the page sent as text."""

    text = body.get(field)
    if not isinstance(text, str):
        raise BackendError(f"'{field}' field is required", status=400)
    if len(text.encode("utf-8")) > _MAX_HISTORY_BYTES:
        raise BackendError("The job record is too large to read", status=413)
    try:
        parsed: object = json.loads(text)
    except ValueError as error:
        raise BackendError(f"'{field}' is not valid JSON", status=400) from error
    return parsed


async def read_job_seeds_route(request: Any) -> Any:
    """Return the seeds of the job whose Comfy reply the page sends as text.

    A finished job is described by its history reply (``history_text``). A job that is still
    waiting or running is described by Comfy's queue reply and its identifier (``queue_text``
    and ``job_id``).

    A page parses numbers as doubles, so a reply it has parsed has already lost the low
    digits of a 64-bit seed. The reply's text is parsed here instead, and each seed is
    returned as its decimal digits.
    """

    try:
        body = await parse_json_body(request)
        seeds: tuple[JobSeed, ...]
        if "queue_text" in body:
            job_id = body.get("job_id")
            if not isinstance(job_id, str) or not job_id:
                raise BackendError("'job_id' field is required", status=400)
            seeds = read_queued_job_seeds(_parsed(body, "queue_text"), job_id)
        else:
            seeds = read_job_seeds(_parsed(body, "history_text"))
        return json_success(
            {
                "seeds": [
                    {
                        "alias": seed.alias,
                        "node_symbol": seed.node_symbol,
                        "input_name": seed.input_name,
                        "digits": seed.digits,
                    }
                    for seed in seeds
                ]
            }
        )
    except BackendError as error:
        return json_error_from_exception(error)


__all__ = ["read_job_seeds_route"]
