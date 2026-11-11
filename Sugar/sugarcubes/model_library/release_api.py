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
"""Expose explicit release history and verified acquisition through thin HTTP adapters."""

from __future__ import annotations

import logging
import re
import sqlite3

import aiohttp
from aiohttp import web

from .infrastructure.runtime import ModelLibraryRuntime
from .infrastructure.model_release_payloads import model_release_history_payload
from .application.enrichment_service import ModelPreviewUnavailableError

_LOGGER = logging.getLogger(__name__)


def register_model_release_routes(
    routes: web.RouteTableDef, runtime: ModelLibraryRuntime
) -> None:
    """Register provider history and acquisition without accepting browser URLs or target paths."""

    async def history(request: web.Request) -> web.Response:
        """Return release choices for an installed model's verified provider identity."""
        try:
            model_id = request.match_info["model_id"]
            family = await runtime.update_feed.browse(runtime.releases, model_id)
            payload = model_release_history_payload(
                runtime.repository, family, model_id
            )
        except (aiohttp.ClientError, TimeoutError):
            _LOGGER.exception("Model provider history request failed")
            return web.json_response(
                {
                    "error": "The model provider connection failed. Check network or certificate settings."
                },
                status=502,
            )
        except ValueError as error:
            return web.json_response({"error": str(error)}, status=400)
        except (OSError, sqlite3.Error):
            _LOGGER.exception("Model release catalog read failed")
            return web.json_response(
                {"error": "Model release history is unavailable."}, status=500
            )
        return web.json_response(payload)

    async def preview(request: web.Request) -> web.StreamResponse:
        """Prepare an independently requested version image without exposing provider URLs."""
        try:
            images = await runtime.release_previews.get(
                request.match_info["model_id"], request.match_info["version_id"]
            )
        except (
            aiohttp.ClientError,
            TimeoutError,
            ValueError,
            OSError,
            ModelPreviewUnavailableError,
        ) as error:
            _LOGGER.warning("Model version preview unavailable: %s", error)
            return web.json_response(
                {"error": "Model version preview is unavailable."}, status=404
            )
        standards = [image for image in images if image.role == "standard"]
        image = max(standards, key=lambda item: item.width, default=None)
        asset = runtime.assets.resolve(image.asset_id) if image else None
        if asset is None:
            return web.json_response(
                {"error": "Model version preview is unavailable."}, status=404
            )
        return web.FileResponse(
            asset.path,
            headers={
                "Content-Type": asset.content_type,
                "Cache-Control": "public, max-age=3600",
                "X-Content-Type-Options": "nosniff",
            },
        )

    async def acquire(request: web.Request) -> web.Response:
        """Queue a provider selection for revalidation rather than trusting caller transport evidence."""
        try:
            payload: object = await request.json()
        except ValueError:
            return web.json_response(
                {"error": "Invalid model acquisition request."}, status=400
            )
        if not isinstance(payload, dict):
            return web.json_response(
                {"error": "Invalid model acquisition request."}, status=400
            )
        version_id, file_id = payload.get("versionId"), payload.get("fileId")
        if (
            not isinstance(version_id, str)
            or not re.fullmatch(r"[1-9][0-9]{0,15}", version_id)
            or not isinstance(file_id, str)
            or not re.fullmatch(r"[1-9][0-9]{0,15}", file_id)
            or set(payload) != {"versionId", "fileId"}
        ):
            return web.json_response(
                {"error": "Invalid model acquisition selection."}, status=400
            )
        job = runtime.acquire_release(
            request.match_info["model_id"], version_id, file_id
        )
        return web.json_response(job.to_payload(), status=202)

    routes.get("/sugarcubes/v1/models/{model_id}/versions")(history)
    routes.get(
        "/sugarcubes/v1/models/{model_id}/versions/{version_id:[1-9][0-9]{0,15}}/preview"
    )(preview)
    routes.post("/sugarcubes/v1/models/{model_id}/acquire")(acquire)
