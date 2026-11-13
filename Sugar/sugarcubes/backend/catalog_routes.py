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

"""Adapt Cube catalog queries to HTTP responses."""

from __future__ import annotations

import logging
from dataclasses import dataclass
from typing import Any

from .composition import BackendServices
from .responses import BackendError, json_error, json_error_from_exception, json_success
from .route_types import RouteHandler

_logger = logging.getLogger(__name__)


@dataclass(frozen=True)
class CatalogRouteHandlers:
    """Collect the endpoint-family handlers for composition."""

    get_status: RouteHandler
    list_cubes: RouteHandler
    list_picker_catalog: RouteHandler


def build_catalog_route_handlers(services: BackendServices) -> CatalogRouteHandlers:
    """Build thin endpoint-family handlers over backend services."""

    async def get_status(request: Any) -> Any:
        """Return SugarCubes availability and installed package identity."""

        _ = request
        return json_success(services.library.library_status(), status=200)

    async def list_cubes(request: Any) -> Any:
        _ = request
        try:
            return json_success(services.library.list_cubes(), status=200)
        except BackendError as error:
            return json_error_from_exception(error)
        except Exception:  # pragma: no cover - defensive
            _logger.exception("SugarCubes: failed to list cubes")
            return json_error("Failed to list SugarCubes", status=500)

    async def list_picker_catalog(request: Any) -> Any:
        """Return host-neutral Cube descriptors for native node discovery."""

        _ = request
        try:
            return json_success(
                services.picker_catalog.list_picker_catalog(), status=200
            )
        except BackendError as error:
            return json_error_from_exception(error)
        except Exception:  # pragma: no cover - defensive
            _logger.exception("SugarCubes: failed to list native picker catalog")
            return json_error("Failed to list SugarCubes picker catalog", status=500)

    return CatalogRouteHandlers(
        get_status=get_status,
        list_cubes=list_cubes,
        list_picker_catalog=list_picker_catalog,
    )


__all__ = ["CatalogRouteHandlers", "build_catalog_route_handlers"]
