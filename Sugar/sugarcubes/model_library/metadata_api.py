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
"""Expose reusable relationships and explicit user maturity choices through typed handlers."""

from __future__ import annotations

import logging
import sqlite3

from aiohttp import web

from .application.content_classification import ModelContentClassification
from .domain.model_relationships import ModelRelationships
from .infrastructure.catalog_updates import ModelCatalogUpdates
from .infrastructure.model_content_codec import optional_nsfw

_LOGGER = logging.getLogger(__name__)


def register_model_metadata_routes(
    routes: web.RouteTableDef,
    relationships: ModelRelationships,
    content: ModelContentClassification,
    updates: ModelCatalogUpdates,
) -> None:
    """Translate bounded requests into the authoritative metadata and invalidation owners."""

    async def read(request: web.Request) -> web.Response:
        """Read cached provider relationships without filesystem or network work."""
        return web.json_response(relationships.to_payload())

    async def configure(request: web.Request) -> web.Response:
        """Preserve provider classification while applying an explicit user override."""
        try:
            value: object = await request.json()
            if not isinstance(value, dict) or set(value) != {"nsfw"}:
                raise ValueError("Invalid model content request.")
            await content.configure(
                request.match_info["model_id"], optional_nsfw(value["nsfw"])
            )
            await updates.publish()
        except ValueError as error:
            return web.json_response({"error": str(error)}, status=400)
        except (OSError, sqlite3.Error):
            _LOGGER.exception("Model content classification could not be saved")
            return web.json_response(
                {"error": "Model classification could not be saved."}, status=500
            )
        return web.json_response({"schemaVersion": 1, "saved": True})

    routes.get("/sugarcubes/v1/models/relationships")(read)
    routes.put("/sugarcubes/v1/models/{model_id}/content")(configure)
