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
"""Expose the host-neutral model library through thin, typed HTTP handlers."""

from __future__ import annotations

import logging
import sqlite3
import re

from aiohttp import web
from PIL import Image, UnidentifiedImageError

from .infrastructure.runtime import ModelLibraryRuntime

_LOGGER = logging.getLogger(__name__)


def register_model_library_routes(
    routes: web.RouteTableDef, runtime: ModelLibraryRuntime
) -> None:
    """Register model catalog, refresh, background jobs, and immutable assets."""

    async def capabilities(request: web.Request) -> web.Response:
        """Let clients negotiate model-library routes independently of the theme."""
        return web.json_response(
            {
                "schemaVersion": 1,
                "owner": "sugarcubes",
                "catalog": "/sugarcubes/v1/models",
                "preparedThumbnails": True,
                "backgroundScan": True,
                "automaticEnrichment": True,
            }
        )

    async def catalog(request: web.Request) -> web.Response:
        """Read prepared catalog data without performing heavy work in the request."""
        try:
            return web.json_response(await runtime.list_models(_kinds(request)))
        except (OSError, ValueError, RuntimeError, sqlite3.Error):
            _LOGGER.exception("Model library catalog read failed")
            return _error("Model catalog is unavailable.", 500)

    async def refresh(request: web.Request) -> web.Response:
        """Queue a coalesced refresh and return its job identifier immediately."""
        return web.json_response(
            runtime.refresh(_kinds(request)).to_payload(), status=202
        )

    async def job_status(request: web.Request) -> web.Response:
        """Return one retained job state without restarting completed work."""
        job = runtime.jobs.get(request.match_info["job_id"])
        return (
            web.json_response(job.to_payload())
            if job
            else _error("Model job was not found.", 404)
        )

    async def cancel_job(request: web.Request) -> web.Response:
        """Cancel an active job through the same owner that scheduled it."""
        job = runtime.jobs.cancel(request.match_info["job_id"])
        return (
            web.json_response(job.to_payload())
            if job
            else _error("Model job was not found.", 404)
        )

    async def changes(request: web.Request) -> web.Response:
        """Long-poll a cheap committed token shared by every model-picker consumer."""
        try:
            after = int(request.query.get("afterRevision", "-1"))
            revision = await runtime.updates.wait(after)
        except ValueError:
            return _error("Invalid model catalog revision.", 400)
        return web.json_response({"schemaVersion": 1, "revision": revision})

    async def refresh_metadata(request: web.Request) -> web.Response:
        """Queue one explicit background provider lookup through the library use case."""
        try:
            job = await runtime.refresh_metadata(request.match_info["model_id"])
        except ValueError:
            return _error("The requested model was not found.", 404)
        return web.json_response(job.to_payload(), status=202)

    async def prioritize_metadata(request: web.Request) -> web.Response:
        """Accept a bounded list of approved identities without reading model bytes in HTTP."""
        try:
            payload: object = await request.json()
        except ValueError:
            return _error("Invalid model priority request.", 400)
        identities: object = (
            payload.get("modelIds") if isinstance(payload, dict) else None
        )
        if (
            not isinstance(identities, list)
            or len(identities) > 128
            or any(
                not isinstance(identity, str)
                or not re.fullmatch(r"[a-f0-9]{64}", identity)
                for identity in identities
            )
        ):
            return _error("Invalid model priority request.", 400)
        normalized = tuple(
            identity for identity in identities if isinstance(identity, str)
        )
        try:
            await runtime.prioritize_metadata(normalized)
        except ValueError:
            return _error("The requested model was not found.", 404)
        return web.json_response(runtime.automatic_enrichment.status(), status=202)

    async def image(request: web.Request) -> web.StreamResponse:
        """Serve prepared immutable bytes using opaque IDs and normal HTTP caching."""
        try:
            asset = runtime.assets.resolve(request.match_info["asset_id"])
        except ValueError:
            return _error("Invalid model image identity.", 400)
        if asset is None:
            return _error("Model image was not found.", 404)
        return web.FileResponse(
            asset.path,
            headers={
                "Content-Type": asset.content_type,
                "Cache-Control": "public, max-age=31536000, immutable",
                "X-Content-Type-Options": "nosniff",
            },
        )

    async def replace_thumbnail(request: web.Request) -> web.Response:
        """Receive bounded image bytes while keeping caller paths and URLs out of asset preparation."""
        content = bytearray()
        async for block in request.content.iter_chunked(64 * 1024):
            if len(content) + len(block) > 20 * 1024 * 1024:
                return _error("Model preview exceeds the 20 MiB limit.", 413)
            content.extend(block)
        try:
            await runtime.replace_thumbnail(
                request.match_info["model_id"], bytes(content)
            )
        except (ValueError, UnidentifiedImageError, Image.DecompressionBombError):
            return _error("The model or preview image is invalid.", 400)
        except (OSError, sqlite3.Error):
            _LOGGER.exception("Model preview replacement failed")
            return _error("Model preview could not be saved.", 500)
        return web.json_response({"schemaVersion": 1, "saved": True})

    routes.get("/sugarcubes/v1/models/capabilities")(capabilities)
    routes.get("/sugarcubes/v1/models")(catalog)
    routes.post("/sugarcubes/v1/models/refresh")(refresh)
    routes.get("/sugarcubes/v1/models/changes")(changes)
    routes.post("/sugarcubes/v1/models/{model_id}/metadata/refresh")(refresh_metadata)
    routes.post("/sugarcubes/v1/models/metadata/prioritize")(prioritize_metadata)
    routes.put("/sugarcubes/v1/models/{model_id}/thumbnail")(replace_thumbnail)
    routes.get("/sugarcubes/v1/models/jobs/{job_id}")(job_status)
    routes.delete("/sugarcubes/v1/models/jobs/{job_id}")(cancel_job)
    routes.get("/sugarcubes/v1/models/assets/{asset_id}")(image)


def _kinds(request: web.Request) -> tuple[str, ...] | None:
    """Normalize optional repeated model-kind query values once at the boundary."""
    values = tuple(value for value in request.query.getall("kind", []) if value)
    return values or None


def _error(message: str, status: int) -> web.Response:
    """Return a stable error envelope suitable for SugarCubes-owned feedback."""
    return web.json_response({"error": message}, status=status)
