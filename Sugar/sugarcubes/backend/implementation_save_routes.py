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

"""Expose thin implementation-save preview and commit HTTP handlers."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Callable, Coroutine

from .composition import BackendServices
from .responses import BackendError, json_error_from_exception, json_success
from .validation import (
    coerce_int,
    normalize_actor,
    normalize_graph_payload,
    normalize_workflow_payload,
    parse_json_body,
    parse_save_many_cube_entries,
)

RouteHandler = Callable[[Any], Coroutine[Any, Any, Any]]


@dataclass(frozen=True)
class ImplementationSaveRouteHandlers:
    """Carry implementation-save route callables for host registration."""

    preview: RouteHandler
    save: RouteHandler


def build_implementation_save_route_handlers(
    services: BackendServices,
) -> ImplementationSaveRouteHandlers:
    """Build preview and commit handlers over one validated request parser."""

    async def preview(request: Any) -> Any:
        """Return a default-change review without mutating cube artifacts."""

        return await _handle(request, services.exporter.preview_implementation)

    async def save(request: Any) -> Any:
        """Commit one fingerprint-validated implementation save."""

        return await _handle(request, services.exporter.save_implementation)

    return ImplementationSaveRouteHandlers(preview=preview, save=save)


async def _handle(request: Any, operation: Callable[..., dict[str, Any]]) -> Any:
    """Parse one implementation-save request and translate expected failures."""

    try:
        body = await parse_json_body(request)
        graph_payload = body.get("graph")
        if graph_payload is None:
            raise BackendError("'graph' field is required", status=400)
        workflow_payload = body.get("workflow")
        if workflow_payload is None:
            raise BackendError("'workflow' field is required", status=400)
        actor = normalize_actor(body.get("actor"))
        return json_success(
            operation(
                graph=normalize_graph_payload(graph_payload),
                workflow=normalize_workflow_payload(workflow_payload),
                workflow_version=coerce_int(body.get("workflow_version"), default=None),
                actor=actor or {},
                cube_entries=parse_save_many_cube_entries(body.get("cubes")),
            ),
            status=200,
        )
    except BackendError as error:
        return json_error_from_exception(error)
