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
"""Adapt SugarScript workflow authoring to a bounded JSON endpoint."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any
from .composition import BackendServices
from .responses import BackendError, json_error_from_exception, json_success
from .route_types import RouteHandler
from .validation import parse_json_body
from .workflow_plan_responses import serialize_sugarscript_result

_MAX_SOURCE_BYTES = 1_000_000


@dataclass(frozen=True)
class SugarScriptRouteHandlers:
    """Collect the additive SugarScript route family."""

    compile_sugarscript: RouteHandler


def build_sugarscript_route_handlers(
    services: BackendServices,
) -> SugarScriptRouteHandlers:
    """Build a thin authoring endpoint over the application service."""

    async def compile_sugarscript(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            source = body.get("source")
            if not isinstance(source, str):
                raise BackendError("'source' field is required", status=400)
            if len(source.encode("utf-8")) > _MAX_SOURCE_BYTES:
                raise BackendError("SugarScript source exceeds 1 MB", status=413)
            result = services.sugarscript_authoring.compile(source)
            return json_success(
                serialize_sugarscript_result(result),
                status=200 if result.is_valid else 422,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    return SugarScriptRouteHandlers(compile_sugarscript=compile_sugarscript)
