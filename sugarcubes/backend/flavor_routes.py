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

"""Adapt local flavor aggregate commands to HTTP responses."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from .composition import BackendServices
from .responses import BackendError, json_error_from_exception, json_success
from .route_types import RouteHandler
from .services.cube_metadata import normalize_metadata_string
from .validation import parse_json_body


@dataclass(frozen=True)
class FlavorRouteHandlers:
    """Collect the endpoint-family handlers for composition."""

    get_local_flavors: RouteHandler
    save_local_flavor: RouteHandler
    delete_local_flavor: RouteHandler
    select_local_flavor: RouteHandler
    migrate_local_flavors: RouteHandler
    reconcile_local_flavors: RouteHandler


def build_flavor_route_handlers(services: BackendServices) -> FlavorRouteHandlers:
    """Build thin endpoint-family handlers over backend services."""

    async def get_local_flavors(request: Any) -> Any:
        try:
            cube_id = request.query.get("cube_id")
            if not isinstance(cube_id, str) or not cube_id.strip():
                raise BackendError("'cube_id' query parameter is required", status=400)
            state = services.local_flavors.read_cube_state(cube_id)
            return json_success({"state": state}, status=200)
        except BackendError as error:
            return json_error_from_exception(error)

    async def save_local_flavor(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            values = body.get("values")
            if not isinstance(values, Mapping):
                raise BackendError("'values' field is required", status=400)
            authored_flavors = body.get("authored_flavors")
            state = services.local_flavors.save_local_flavor(
                cube_id=str(body.get("cube_id") or ""),
                surface_signature=str(body.get("surface_signature") or ""),
                name=str(body.get("name") or ""),
                values=values,
                flavor_id=normalize_metadata_string(body.get("flavor_id")) or None,
                authored_flavors=(
                    authored_flavors if isinstance(authored_flavors, list) else []
                ),
            )
            return json_success({"state": state}, status=200)
        except BackendError as error:
            return json_error_from_exception(error)

    async def delete_local_flavor(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            state = services.local_flavors.delete_local_flavor(
                cube_id=str(body.get("cube_id") or ""),
                surface_signature=str(body.get("surface_signature") or ""),
                flavor_id=str(body.get("flavor_id") or ""),
            )
            return json_success({"state": state}, status=200)
        except BackendError as error:
            return json_error_from_exception(error)

    async def select_local_flavor(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            state = services.local_flavors.set_selected_flavor(
                cube_id=str(body.get("cube_id") or ""),
                surface_signature=str(body.get("surface_signature") or ""),
                flavor_id=str(body.get("flavor_id") or ""),
            )
            return json_success({"state": state}, status=200)
        except BackendError as error:
            return json_error_from_exception(error)

    async def migrate_local_flavors(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            states = body.get("states")
            if not isinstance(states, list):
                raise BackendError("'states' field is required", status=400)
            payload = services.local_flavors.migrate_states(states)
            return json_success(payload, status=200)
        except BackendError as error:
            return json_error_from_exception(error)

    async def reconcile_local_flavors(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            authored_flavors = body.get("authored_flavors")
            rename_map = body.get("rename_map")
            payload = services.local_flavors.reconcile_with_authored_flavors(
                cube_id=str(body.get("cube_id") or ""),
                surface_signature=str(body.get("surface_signature") or ""),
                authored_flavors=(
                    authored_flavors if isinstance(authored_flavors, list) else []
                ),
                rename_map=rename_map if isinstance(rename_map, Mapping) else None,
            )
            return json_success(payload, status=200)
        except BackendError as error:
            return json_error_from_exception(error)

    return FlavorRouteHandlers(
        get_local_flavors=get_local_flavors,
        save_local_flavor=save_local_flavor,
        delete_local_flavor=delete_local_flavor,
        select_local_flavor=select_local_flavor,
        migrate_local_flavors=migrate_local_flavors,
        reconcile_local_flavors=reconcile_local_flavors,
    )


__all__ = ["FlavorRouteHandlers", "build_flavor_route_handlers"]
