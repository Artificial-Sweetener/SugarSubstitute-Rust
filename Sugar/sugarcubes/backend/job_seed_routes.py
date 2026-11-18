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
from typing import Any

from ..authoring.job_seeds import read_job_seeds
from .responses import BackendError, json_error_from_exception, json_success
from .validation import parse_json_body

_MAX_HISTORY_BYTES = 32_000_000


async def read_job_seeds_route(request: Any) -> Any:
    """Return the seeds of the job whose Comfy history reply the page sends as text.

    A page parses numbers as doubles, so a reply it has parsed has already lost the low
    digits of a 64-bit seed. The reply's text is parsed here instead, and each seed is
    returned as its decimal digits.
    """

    try:
        body = await parse_json_body(request)
        text = body.get("history_text")
        if not isinstance(text, str):
            raise BackendError("'history_text' field is required", status=400)
        if len(text.encode("utf-8")) > _MAX_HISTORY_BYTES:
            raise BackendError("The job record is too large to read", status=413)
        try:
            history = json.loads(text)
        except ValueError as error:
            raise BackendError(
                "'history_text' is not valid JSON", status=400
            ) from error
        return json_success(
            {
                "seeds": [
                    {
                        "alias": seed.alias,
                        "node_symbol": seed.node_symbol,
                        "input_name": seed.input_name,
                        "digits": seed.digits,
                    }
                    for seed in read_job_seeds(history)
                ]
            }
        )
    except BackendError as error:
        return json_error_from_exception(error)


__all__ = ["read_job_seeds_route"]
