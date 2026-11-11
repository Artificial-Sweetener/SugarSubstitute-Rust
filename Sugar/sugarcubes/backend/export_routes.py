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

"""Adapt Cube export and authored-flavor workflows to HTTP responses."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from .composition import BackendServices
from .responses import BackendError, json_error_from_exception, json_success
from .route_types import RouteHandler
from .services.cube_metadata import normalize_metadata_string
from .validation import (
    coerce_int,
    normalize_actor,
    normalize_graph_payload,
    normalize_workflow_payload,
    parse_json_body,
    parse_save_many_cube_entries,
)


@dataclass(frozen=True)
class ExportRouteHandlers:
    """Collect the endpoint-family handlers for composition."""

    save_many: RouteHandler
    save_authored_flavor: RouteHandler


def build_export_route_handlers(services: BackendServices) -> ExportRouteHandlers:
    """Build thin endpoint-family handlers over backend services."""

    async def save_many(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            graph_payload = body.get("graph")
            if graph_payload is None:
                raise BackendError("'graph' field is required", status=400)
            workflow_raw = body.get("workflow")
            if workflow_raw is None:
                raise BackendError("'workflow' field is required", status=400)
            actor = normalize_actor(body.get("actor"))
            return json_success(
                services.exporter.save_many(
                    graph=normalize_graph_payload(graph_payload),
                    workflow=normalize_workflow_payload(workflow_raw),
                    workflow_version=coerce_int(
                        body.get("workflow_version"), default=None
                    ),
                    actor=actor or {},
                    cube_entries=parse_save_many_cube_entries(body.get("cubes")),
                ),
                status=200,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    async def save_authored_flavor(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            values = body.get("values")
            if not isinstance(values, Mapping):
                raise BackendError("'values' field is required", status=400)
            return json_success(
                services.exporter.save_authored_flavor(
                    cube_id=body.get("cube_id", ""),
                    values=values,
                    flavor_id=normalize_metadata_string(body.get("flavor_id")),
                    flavor_name=normalize_metadata_string(body.get("flavor_name")),
                ),
                status=200,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    return ExportRouteHandlers(
        save_many=save_many,
        save_authored_flavor=save_authored_flavor,
    )


__all__ = ["ExportRouteHandlers", "build_export_route_handlers"]
