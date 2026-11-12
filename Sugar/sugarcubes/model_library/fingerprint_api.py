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
"""Expose verified local identity without coupling desktop consumers to providers."""

from __future__ import annotations

import asyncio
import logging
import re
import sqlite3

from aiohttp import web

from .infrastructure.runtime import ModelLibraryRuntime

_LOGGER = logging.getLogger(__name__)


def register_model_fingerprint_routes(
    routes: web.RouteTableDef, runtime: ModelLibraryRuntime
) -> None:
    """Register explicit fingerprint jobs and cheap indexed identity queries."""

    async def refresh(request: web.Request) -> web.Response:
        """Queue approved file hashing without requesting external model metadata."""
        try:
            job = await runtime.refresh_fingerprint(request.match_info["model_id"])
        except ValueError:
            return web.json_response(
                {"error": "The requested model was not found."}, status=404
            )
        except (OSError, sqlite3.Error):
            _LOGGER.exception("Model fingerprint request failed")
            return web.json_response(
                {"error": "Model fingerprint work is unavailable."}, status=500
            )
        return web.json_response(job.to_payload(), status=202)

    async def by_hash(request: web.Request) -> web.Response:
        """Return only cached verified matches; unknown files are never hashed implicitly."""
        digest = request.match_info["sha256"].lower()
        kind = request.query.get("kind", "")
        if re.fullmatch(r"[a-f0-9]{64}", digest) is None or not kind or len(kind) > 128:
            return web.json_response(
                {"error": "A model kind and SHA256 are required."}, status=400
            )
        try:
            snapshot = await asyncio.to_thread(runtime.repository.snapshot, (kind,))
        except (OSError, ValueError, sqlite3.Error):
            _LOGGER.exception("Model identity query failed", extra={"kind": kind})
            return web.json_response(
                {"error": "Model identities are unavailable."}, status=500
            )
        return web.json_response(
            {
                "schemaVersion": 1,
                "revision": snapshot.revision,
                "models": [
                    entry.to_payload()
                    for entry in snapshot.entries
                    if entry.sha256 == digest
                ],
                "unfingerprintedCount": sum(
                    entry.sha256 is None for entry in snapshot.entries
                ),
            }
        )

    routes.get("/sugarcubes/v1/models/by-hash/{sha256}")(by_hash)
    routes.post("/sugarcubes/v1/models/{model_id}/fingerprint/refresh")(refresh)
