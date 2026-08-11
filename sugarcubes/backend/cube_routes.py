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

"""Adapt Cube revision, artifact, metadata, and import workflows to HTTP."""

from __future__ import annotations

import logging
from dataclasses import dataclass
from typing import Any, Mapping

from aiohttp import web

from ..importer import CubeImportError
from .composition import BackendServices
from .responses import BackendError, json_error, json_error_from_exception, json_success
from .route_types import RouteHandler
from .services.cube_metadata import normalize_metadata_string
from .validation import (
    extract_drop_origin,
    get_bool,
    parse_json_body,
    parse_optional_json_body,
)

_logger = logging.getLogger(__name__)


@dataclass(frozen=True)
class CubeRouteHandlers:
    """Collect the endpoint-family handlers for composition."""

    list_revisions: RouteHandler
    load_revision: RouteHandler
    preview_cube: RouteHandler
    serve_icon_asset: RouteHandler
    load_cube: RouteHandler
    update_metadata: RouteHandler
    rename_cube: RouteHandler
    promote_cube: RouteHandler
    delete_cube: RouteHandler
    import_cube_file: RouteHandler


def build_cube_route_handlers(services: BackendServices) -> CubeRouteHandlers:
    """Build thin endpoint-family handlers over backend services."""

    async def list_revisions(request: Any) -> Any:
        cube_id = request.query.get("cube_id")
        if not isinstance(cube_id, str) or not cube_id.strip():
            return json_error("'cube_id' query parameter is required", status=400)
        try:
            return json_success(
                services.revisions.list_revisions(cube_id=cube_id),
                status=200,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    async def load_revision(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            return json_success(
                services.revisions.load_revision(
                    cube_id=body.get("cube_id", ""),
                    revision_ref=body.get("revision_ref", ""),
                    version_pin=normalize_metadata_string(body.get("version_pin")),
                    drop_origin=extract_drop_origin(body.get("origin")) or (0.0, 0.0),
                ),
                status=200,
            )
        except CubeImportError as exc:
            return json_error(exc.message, status=400, details=exc.details or None)
        except BackendError as error:
            return json_error_from_exception(error)

    async def preview_cube(request: Any) -> Any:
        cube_id = request.query.get("cube_id")
        if not isinstance(cube_id, str) or not cube_id.strip():
            return json_error("'cube_id' query parameter is required", status=400)
        try:
            return json_success(services.library.preview_cube(cube_id), status=200)
        except CubeImportError as exc:
            return json_error(exc.message, status=400, details=exc.details or None)
        except BackendError as error:
            return json_error_from_exception(error)

    async def serve_icon_asset(request: Any) -> Any:
        cube_id = request.query.get("cube_id")
        if not isinstance(cube_id, str) or not cube_id.strip():
            return json_error("'cube_id' query parameter is required", status=400)
        try:
            icon_path, media_type = services.library.resolve_cube_icon_asset(cube_id)
            return web.Response(
                body=icon_path.read_bytes(),
                content_type=media_type,
            )
        except BackendError as error:
            return json_error_from_exception(error)
        except OSError:
            _logger.exception("SugarCubes: failed to read cube icon asset")
            return json_error("Failed to read cube icon asset", status=500)

    async def load_cube(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            return json_success(
                services.loader.load_cube(
                    cube_id=body.get("cube_id", ""),
                    version_pin=normalize_metadata_string(body.get("version_pin")),
                    drop_origin=extract_drop_origin(body.get("origin")) or (0.0, 0.0),
                ),
                status=200,
            )
        except CubeImportError as exc:
            return json_error(exc.message, status=400, details=exc.details or None)
        except BackendError as error:
            return json_error_from_exception(error)

    async def update_metadata(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            metadata_value = body.get("metadata")
            return json_success(
                services.metadata.update_metadata(
                    cube_id=body.get("cube_id", ""),
                    description_set="description" in body,
                    description=normalize_metadata_string(body.get("description")),
                    version_set="version" in body,
                    version=normalize_metadata_string(body.get("version")),
                    metadata_payload=(
                        metadata_value if isinstance(metadata_value, Mapping) else {}
                    ),
                ),
                status=200,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    async def rename_cube(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            if body.get("derive_target_from_name") is True:
                return json_success(
                    services.metadata.rename_cube_from_default_alias(
                        cube_id=body.get("cube_id", ""),
                        target_default_alias=normalize_metadata_string(
                            body.get("default_alias")
                        ),
                    ),
                    status=200,
                )
            return json_success(
                services.metadata.rename_cube(
                    cube_id=body.get("cube_id", ""),
                    target_cube_id=body.get("target_cube_id", ""),
                    target_default_alias=normalize_metadata_string(
                        body.get("default_alias")
                    ),
                ),
                status=200,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    async def promote_cube(request: Any) -> Any:
        """Move one personal cube into a claimed writable cube pack."""

        try:
            body = await parse_json_body(request)
            destination = body.get("destination")
            if not isinstance(destination, Mapping):
                raise BackendError("'destination' field is required", status=400)
            return json_success(
                services.promotion.promote(
                    source_cube_id=str(body.get("source_cube_id") or ""),
                    owner=str(destination.get("owner") or ""),
                    repo=str(destination.get("repo") or ""),
                    name=str(body.get("name") or ""),
                    target_model=str(body.get("target_model") or ""),
                    supported_models=body.get("supported_models"),
                    description_set="description" in body,
                    description=normalize_metadata_string(body.get("description")),
                    metadata=(
                        body.get("metadata")
                        if isinstance(body.get("metadata"), Mapping)
                        else {}
                    ),
                ),
                status=200,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    async def delete_cube(request: Any) -> Any:
        cube_id = request.query.get("cube_id")
        try:
            body = None
            if not isinstance(cube_id, str) or not cube_id.strip():
                body = await parse_optional_json_body(request)
            if isinstance(body, Mapping):
                cube_id = body.get("cube_id")
            normalized_cube_id = normalize_metadata_string(cube_id)
            if not normalized_cube_id:
                return json_error("'cube_id' is required", status=400)
            return json_success(
                services.library.delete_cube(
                    cube_id=normalized_cube_id,
                ),
                status=200,
            )
        except BackendError as error:
            return json_error_from_exception(error)

    async def import_cube_file(request: Any) -> Any:
        try:
            body = await parse_json_body(request)
            source_value = body.get("path") or body.get("source") or body.get("file")
            if not isinstance(source_value, str) or not source_value.strip():
                raise BackendError("'path' field is required", status=400)
            target_cube_id = body.get("cube_id") or body.get("target_cube_id")
            if not isinstance(target_cube_id, str) or not target_cube_id.strip():
                raise BackendError("'cube_id' field is required", status=400)
            return json_success(
                services.library.import_cube_file(
                    source_value=source_value,
                    target_cube_id=target_cube_id,
                    overwrite=get_bool(body, "overwrite", False),
                ),
                status=201,
            )
        except CubeImportError as exc:
            return json_error(exc.message, status=400, details=exc.details or None)
        except BackendError as error:
            return json_error_from_exception(error)

    return CubeRouteHandlers(
        list_revisions=list_revisions,
        load_revision=load_revision,
        preview_cube=preview_cube,
        serve_icon_asset=serve_icon_asset,
        load_cube=load_cube,
        update_metadata=update_metadata,
        rename_cube=rename_cube,
        promote_cube=promote_cube,
        delete_cube=delete_cube,
        import_cube_file=import_cube_file,
    )


__all__ = ["CubeRouteHandlers", "build_cube_route_handlers"]
